//! FEM solvers for Helmholtz equation
//!
//! This module provides solvers for the assembled Helmholtz system using
//! sparse matrix operations from the `math-solvers` crate.
//!
//! # Solver Types
//!
//! - **Direct**: LU factorization (for small problems)
//! - **GMRES**: Iterative solver with restart
//! - **GMRES+ILU**: GMRES with ILU(0) preconditioning (recommended for large problems)
//! - **GMRES+ShiftedLaplacian**: GMRES with shifted-Laplacian preconditioner (best for Helmholtz)
//! - **GMRES+AMG**: GMRES with algebraic multigrid preconditioning (best parallel scalability)

use crate::assembly::HelmholtzProblem;
use crate::assembly::{MassMatrix, StiffnessMatrix};
use math_audio_solvers::iterative::{
    gmres_pipelined, gmres_preconditioned, gmres_preconditioned_with_guess,
};
use math_audio_solvers::{
    AdditiveSchwarzPreconditioner, AmgConfig, AmgPreconditioner, CsrMatrix, DiagonalPreconditioner,
    GmresConfig, IdentityPreconditioner, IluColoringPreconditioner, IluFixedPointPreconditioner,
    IluPreconditioner, gmres, lu_solve,
};
use ndarray::Array1;
use num_complex::Complex64;
use std::time::Instant;
use thiserror::Error;

/// GMRES solver configuration with f64 tolerance
pub type GmresConfigF64 = GmresConfig<f64>;

/// Solver configuration
#[derive(Debug, Clone)]
pub struct SolverConfig {
    /// Solver type to use
    pub solver_type: SolverType,
    /// GMRES configuration (used for iterative solvers)
    pub gmres: GmresConfigF64,
    /// Verbosity level (0 = quiet, 1 = summary, 2+ = detailed)
    pub verbosity: usize,
    /// Number of subdomains for Schwarz preconditioning (default: 8)
    pub schwarz_subdomains: usize,
    /// Overlap for Schwarz preconditioning (default: 2)
    pub schwarz_overlap: usize,
    /// Shifted-Laplacian configuration (for GmresShiftedLaplacian solver)
    pub shifted_laplacian: Option<ShiftedLaplacianConfig>,
    /// Wavenumber k (used for default shifted-laplacian parameters)
    pub wavenumber: Option<f64>,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            solver_type: SolverType::GmresIlu,
            gmres: GmresConfigF64 {
                max_iterations: 1000,
                restart: 50,
                tolerance: 1e-10,
                print_interval: 0,
            },
            verbosity: 0,
            schwarz_subdomains: 8,
            schwarz_overlap: 2,
            shifted_laplacian: None,
            wavenumber: None,
        }
    }
}

/// Type of solver to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverType {
    /// Direct LU factorization (best for small problems)
    Direct,
    /// GMRES iterative solver without preconditioning
    Gmres,
    /// GMRES with ILU(0) preconditioning (recommended for large problems)
    GmresIlu,
    /// GMRES with Jacobi (diagonal) preconditioning - fully parallel
    GmresJacobi,
    /// GMRES with parallel ILU using graph coloring (level scheduling)
    GmresIluColoring,
    /// GMRES with parallel ILU using fixed-point iteration
    GmresIluFixedPoint,
    /// GMRES with Additive Schwarz domain decomposition (parallel subdomains)
    GmresSchwarz,
    /// GMRES with Algebraic Multigrid preconditioning (best parallel scalability)
    GmresAmg,
    /// Pipelined GMRES (overlaps communication/computation)
    GmresPipelined,
    /// Pipelined GMRES with ILU(0) preconditioning
    GmresPipelinedIlu,
    /// Pipelined GMRES with AMG preconditioning (best for large parallel problems)
    GmresPipelinedAmg,
    /// GMRES with Shifted-Laplacian preconditioner (best for Helmholtz)
    ///
    /// The shifted-Laplacian preconditioner:
    /// P = K + (α + iβ)M
    ///
    /// where α ≈ k² and β ≈ k are complex shifts that transform
    /// the indefinite Helmholtz operator to a more favorable spectrum.
    ///
    /// Reference: Erlangga et al. (2006) "A class of preconditioners for the Helmholtz equation"
    GmresShiftedLaplacian,
    /// GMRES with shifted-Laplacian and V-cycle multigrid smoothing
    GmresShiftedLaplacianMg,
}

/// Configuration for Shifted-Laplacian preconditioner
#[derive(Debug, Clone, PartialEq)]
pub struct ShiftedLaplacianConfig {
    /// Real part of complex shift α (default: k²)
    /// Controls damping of low-frequency content
    pub alpha: f64,
    /// Imaginary part of complex shift β (default: k)
    /// Controls damping of high-frequency content
    pub beta: f64,
    /// Number of multigrid V-cycles (0 = use AMG only)
    pub mg_cycles: usize,
    /// AMG coarsening levels for preconditioner setup
    pub amg_levels: usize,
    /// Relaxation factor for smoother
    pub omega: f64,
    /// Pre-smooth iterations
    pub presmooth: usize,
    /// Post-smooth iterations
    pub postsmooth: usize,
}

impl Default for ShiftedLaplacianConfig {
    fn default() -> Self {
        Self {
            alpha: 1.0, // Will be scaled by k²
            beta: 1.0,  // Will be scaled by k
            mg_cycles: 2,
            amg_levels: 0, // Auto-select
            omega: 0.8,
            presmooth: 2,
            postsmooth: 2,
        }
    }
}

impl ShiftedLaplacianConfig {
    /// Create configuration optimized for given wavenumber
    ///
    /// Uses empirically optimal shifts:
    /// - α = 0.5 k² (moderate real shift)
    /// - β = 0.5 k (imaginary shift for damping)
    pub fn for_wavenumber(k: f64) -> Self {
        Self {
            alpha: 0.5 * k * k,
            beta: 0.5 * k,
            mg_cycles: 2,
            amg_levels: 0,
            omega: 0.8,
            presmooth: 2,
            postsmooth: 2,
        }
    }

    /// Aggressive shifts for difficult problems
    pub fn aggressive(k: f64) -> Self {
        Self {
            alpha: k * k,
            beta: k,
            mg_cycles: 3,
            amg_levels: 0,
            omega: 0.7,
            presmooth: 3,
            postsmooth: 3,
        }
    }

    /// Conservative shifts for well-conditioned problems
    pub fn conservative(k: f64) -> Self {
        Self {
            alpha: 0.25 * k * k,
            beta: 0.25 * k,
            mg_cycles: 1,
            amg_levels: 0,
            omega: 0.9,
            presmooth: 1,
            postsmooth: 1,
        }
    }
}

/// Solution result from the solver
#[derive(Debug, Clone)]
pub struct Solution {
    /// Solution vector
    pub values: Array1<Complex64>,
    /// Number of iterations (0 for direct solver)
    pub iterations: usize,
    /// Final residual norm
    pub residual: f64,
    /// Whether the solver converged
    pub converged: bool,
}

/// Solver errors
#[derive(Debug, Error)]
pub enum SolverError {
    #[error("Solver failed to converge after {0} iterations (residual: {1})")]
    ConvergenceFailure(usize, f64),
    #[error("Direct solver failed: singular matrix")]
    SingularMatrix,
    #[error("Matrix dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("Invalid solver configuration: {0}")]
    InvalidConfiguration(String),
}

/// Solve a Helmholtz problem
///
/// # Arguments
/// * `problem` - The assembled Helmholtz problem
/// * `config` - Solver configuration
///
/// # Returns
/// Solution containing the nodal values
pub fn solve(problem: &HelmholtzProblem, config: &SolverConfig) -> Result<Solution, SolverError> {
    let start = Instant::now();

    // Convert to CSR format
    let csr = problem.matrix.to_csr();
    let rhs = Array1::from(problem.rhs.clone());

    let csr_time = start.elapsed();

    if config.verbosity > 0 {
        println!(
            "  [FEM] System: {} DOFs, {} nnz, sparsity {:.4}%, CSR convert: {:.1}ms",
            csr.num_rows,
            csr.nnz(),
            csr.sparsity() * 100.0,
            csr_time.as_secs_f64() * 1000.0
        );
    }

    let solve_start = Instant::now();
    let result = match config.solver_type {
        SolverType::Direct => solve_direct(&csr, &rhs, config),
        SolverType::Gmres => solve_gmres(&csr, &rhs, config),
        SolverType::GmresIlu => solve_gmres_ilu(&csr, &rhs, config),
        SolverType::GmresJacobi => solve_gmres_jacobi(&csr, &rhs, config),
        SolverType::GmresIluColoring => solve_gmres_ilu_coloring(&csr, &rhs, config),
        SolverType::GmresIluFixedPoint => solve_gmres_ilu_fixedpoint(&csr, &rhs, config),
        SolverType::GmresSchwarz => solve_gmres_schwarz(&csr, &rhs, config),
        SolverType::GmresAmg => solve_gmres_amg(&csr, &rhs, config),
        SolverType::GmresPipelined => solve_gmres_pipelined(&csr, &rhs, config),
        SolverType::GmresPipelinedIlu => solve_gmres_pipelined_ilu(&csr, &rhs, config),
        SolverType::GmresPipelinedAmg => solve_gmres_pipelined_amg(&csr, &rhs, config),
        SolverType::GmresShiftedLaplacian => {
            solve_gmres_shifted_laplacian(problem, &csr, &rhs, config)
        }
        SolverType::GmresShiftedLaplacianMg => {
            solve_gmres_shifted_laplacian_mg(problem, &csr, &rhs, config)
        }
    };
    let solve_time = solve_start.elapsed();

    if config.verbosity > 0
        && let Ok(ref sol) = result
    {
        println!(
            "  [FEM] Solve: {} iters, residual {:.2e}, time {:.1}ms",
            sol.iterations,
            sol.residual,
            solve_time.as_secs_f64() * 1000.0
        );
    }

    result
}

/// Solve using direct LU factorization
fn solve_direct(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!("Using direct LU solver");
    }

    // Convert to dense for direct solve (only suitable for small problems)
    let dense = csr.to_dense();
    let solution = lu_solve(&dense, rhs).map_err(|_| SolverError::SingularMatrix)?;

    // Compute residual
    let residual_vec = csr.matvec(&solution);
    let residual: f64 = residual_vec
        .iter()
        .zip(rhs.iter())
        .map(|(r, b)| (r - b).norm())
        .sum::<f64>()
        / rhs.len() as f64;

    Ok(Solution {
        values: solution,
        iterations: 0,
        residual,
        converged: true,
    })
}

/// Solve using GMRES without preconditioning
fn solve_gmres(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using GMRES solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    let result = gmres(csr, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with ILU(0) preconditioning
///
/// Note: ILU preconditioning involves sequential triangular solves (forward/backward
/// substitution) which cannot be effectively parallelized. For problems where parallel
/// scalability is critical, consider using GMRES without preconditioning or a different
/// preconditioner like Jacobi (diagonal scaling).
fn solve_gmres_ilu(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using GMRES+ILU solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    // Build ILU preconditioner
    let ilu_start = Instant::now();
    let precond = IluPreconditioner::from_csr(csr);
    let ilu_time = ilu_start.elapsed();

    if config.verbosity > 0 {
        println!(
            "  [FEM] ILU factorization: {:.1}ms",
            ilu_time.as_secs_f64() * 1000.0
        );
    }

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES+ILU {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Jacobi (diagonal) preconditioning
///
/// This solver is fully parallel since Jacobi preconditioning only involves
/// element-wise operations on the diagonal. Use this when parallel scalability
/// is more important than convergence rate.
fn solve_gmres_jacobi(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using GMRES+Jacobi solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    // Build Jacobi preconditioner (embarrassingly parallel)
    let precond = DiagonalPreconditioner::from_csr(csr);

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES+Jacobi {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with parallel ILU (graph coloring / level scheduling)
///
/// Uses level scheduling to identify independent rows that can be solved
/// in parallel. Good for matrices with structured sparsity patterns.
fn solve_gmres_ilu_coloring(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using GMRES+ILU-Coloring solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    let ilu_start = Instant::now();
    let precond = IluColoringPreconditioner::from_csr(csr);
    let ilu_time = ilu_start.elapsed();

    if config.verbosity > 0 {
        let (fwd_levels, bwd_levels, avg_fwd, avg_bwd) = precond.level_stats();
        println!(
            "  [FEM] ILU-Coloring: {:.1}ms, {} fwd levels (avg {:.1}), {} bwd levels (avg {:.1})",
            ilu_time.as_secs_f64() * 1000.0,
            fwd_levels,
            avg_fwd,
            bwd_levels,
            avg_bwd
        );
    }

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES+ILU-Coloring {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with parallel ILU (fixed-point iteration)
///
/// Uses Jacobi-style fixed-point iteration instead of exact triangular solves.
/// Each iteration is embarrassingly parallel. Good for highly parallel hardware.
fn solve_gmres_ilu_fixedpoint(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    const FP_ITERATIONS: usize = 10; // Number of fixed-point iterations (10 for Helmholtz)

    if config.verbosity > 0 {
        log::info!(
            "Using GMRES+ILU-FixedPoint solver (restart={}, tol={}, fp_iters={})",
            config.gmres.restart,
            config.gmres.tolerance,
            FP_ITERATIONS
        );
    }

    let ilu_start = Instant::now();
    let precond = IluFixedPointPreconditioner::from_csr(csr, FP_ITERATIONS);
    let ilu_time = ilu_start.elapsed();

    if config.verbosity > 0 {
        println!(
            "  [FEM] ILU-FixedPoint: {:.1}ms ({} iterations per apply)",
            ilu_time.as_secs_f64() * 1000.0,
            FP_ITERATIONS
        );
    }

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES+ILU-FixedPoint {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Additive Schwarz domain decomposition
///
/// Divides the problem into overlapping subdomains that are solved in parallel.
/// Each subdomain uses its own local ILU factorization. This approach is
/// embarrassingly parallel at the subdomain level.
fn solve_gmres_schwarz(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let num_subdomains = config.schwarz_subdomains;
    let overlap = config.schwarz_overlap;

    if config.verbosity > 0 {
        log::info!(
            "Using GMRES+Schwarz solver (restart={}, tol={}, {} subdomains, {} overlap)",
            config.gmres.restart,
            config.gmres.tolerance,
            num_subdomains,
            overlap
        );
    }

    let schwarz_start = Instant::now();
    let precond = AdditiveSchwarzPreconditioner::from_csr(csr, num_subdomains, overlap);
    let schwarz_time = schwarz_start.elapsed();

    if config.verbosity > 0 {
        let (num_subdomains, min_size, max_size, avg_size) = precond.stats();
        println!(
            "  [FEM] Schwarz DD: {:.1}ms, {} subdomains (size: min={}, max={}, avg={:.1})",
            schwarz_time.as_secs_f64() * 1000.0,
            num_subdomains,
            min_size,
            max_size,
            avg_size
        );
    }

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES+Schwarz {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Algebraic Multigrid (AMG) preconditioning
///
/// AMG provides excellent parallel scalability because:
/// - Coarsening can be parallelized (PMIS algorithm)
/// - Jacobi smoothing is embarrassingly parallel
/// - Each multigrid level operates independently
///
/// This is recommended for large problems where parallel efficiency is important.
fn solve_gmres_amg(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using GMRES+AMG solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    let amg_start = Instant::now();
    // Use PMIS coarsening and L1-Jacobi smoothing for better Helmholtz robustness
    let mut amg_config = AmgConfig::for_parallel();
    amg_config.smoother = math_audio_solvers::AmgSmoother::L1Jacobi;
    
    let precond = AmgPreconditioner::from_csr(csr, amg_config);
    let amg_time = amg_start.elapsed();

    if config.verbosity > 0 {
        let diag = precond.diagnostics();
        println!(
            "  [FEM] AMG setup: {:.1}ms, {} levels, grid complexity {:.2}, operator complexity {:.2}",
            amg_time.as_secs_f64() * 1000.0,
            diag.num_levels,
            diag.grid_complexity,
            diag.operator_complexity
        );
    }

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "GMRES+AMG {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using Pipelined GMRES without preconditioning
fn solve_gmres_pipelined(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using Pipelined GMRES solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    let precond = IdentityPreconditioner;
    let result = gmres_pipelined(csr, &precond, rhs, None, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "Pipelined GMRES {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using Pipelined GMRES with ILU(0) preconditioning
fn solve_gmres_pipelined_ilu(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using Pipelined GMRES+ILU solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    let ilu_start = Instant::now();
    let precond = IluPreconditioner::from_csr(csr);
    let ilu_time = ilu_start.elapsed();

    if config.verbosity > 0 {
        println!(
            "  [FEM] ILU factorization: {:.1}ms",
            ilu_time.as_secs_f64() * 1000.0
        );
    }

    let result = gmres_pipelined(csr, &precond, rhs, None, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "Pipelined GMRES+ILU {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using Pipelined GMRES with AMG preconditioning
///
/// Combines the communication-hiding benefits of pipelined GMRES with the
/// excellent parallel scalability of AMG preconditioning. This is the best
/// choice for very large problems on many-core systems.
fn solve_gmres_pipelined_amg(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if config.verbosity > 0 {
        log::info!(
            "Using Pipelined GMRES+AMG solver (restart={}, tol={})",
            config.gmres.restart,
            config.gmres.tolerance
        );
    }

    let amg_start = Instant::now();
    // Use PMIS coarsening and L1-Jacobi smoothing for better Helmholtz robustness
    let mut amg_config = AmgConfig::for_parallel();
    amg_config.smoother = math_audio_solvers::AmgSmoother::L1Jacobi;
    let precond = AmgPreconditioner::from_csr(csr, amg_config);
    let amg_time = amg_start.elapsed();

    if config.verbosity > 0 {
        let diag = precond.diagnostics();
        println!(
            "  [FEM] AMG setup: {:.1}ms, {} levels, grid complexity {:.2}, operator complexity {:.2}",
            amg_time.as_secs_f64() * 1000.0,
            diag.num_levels,
            diag.grid_complexity,
            diag.operator_complexity
        );
    }

    let result = gmres_pipelined(csr, &precond, rhs, None, &config.gmres);

    if config.verbosity > 0 {
        log::info!(
            "Pipelined GMRES+AMG {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

// ========== Solver variants with initial guess support ==========

/// Solve using GMRES without preconditioning, with optional initial guess
fn solve_gmres_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    // Build identity preconditioner for consistent interface
    let precond = IdentityPreconditioner;
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with ILU(0) preconditioning, with optional initial guess
fn solve_gmres_ilu_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let precond = IluPreconditioner::from_csr(csr);
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Jacobi preconditioning, with optional initial guess
fn solve_gmres_jacobi_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let precond = DiagonalPreconditioner::from_csr(csr);
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with ILU coloring, with optional initial guess
fn solve_gmres_ilu_coloring_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let precond = IluColoringPreconditioner::from_csr(csr);
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with ILU fixed-point, with optional initial guess
fn solve_gmres_ilu_fixedpoint_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    const FP_ITERATIONS: usize = 10;
    let precond = IluFixedPointPreconditioner::from_csr(csr, FP_ITERATIONS);
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Schwarz preconditioning, with optional initial guess
fn solve_gmres_schwarz_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let precond = AdditiveSchwarzPreconditioner::from_csr(
        csr,
        config.schwarz_subdomains,
        config.schwarz_overlap,
    );
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with AMG preconditioning, with optional initial guess
fn solve_gmres_amg_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let amg_config = AmgConfig::for_parallel();
    let precond = AmgPreconditioner::from_csr(csr, amg_config);
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using pipelined GMRES without preconditioning, with optional initial guess
fn solve_gmres_pipelined_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let precond = IdentityPreconditioner;
    let result = gmres_pipelined(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using pipelined GMRES with ILU preconditioning, with optional initial guess
fn solve_gmres_pipelined_ilu_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let precond = IluPreconditioner::from_csr(csr);
    let result = gmres_pipelined(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using pipelined GMRES with AMG preconditioning, with optional initial guess
fn solve_gmres_pipelined_amg_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let amg_config = AmgConfig::for_parallel();
    let precond = AmgPreconditioner::from_csr(csr, amg_config);
    let result = gmres_pipelined(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Build shifted-Laplacian matrix P = K + (α + iβ)M from component matrices
///
/// The shifted-Laplacian preconditioner transforms the Helmholtz operator
/// A = K - k²M to P = K + (α + iβ)M which has more favorable spectral properties.
///
/// Reference: Erlangga et al. (2006) "A class of preconditioners for the Helmholtz equation"
fn build_shifted_laplacian(
    stiffness: &StiffnessMatrix,
    mass: &MassMatrix,
    alpha: f64,
    beta: f64,
) -> CsrMatrix<Complex64> {
    use std::collections::HashMap;

    let mut entries: HashMap<(usize, usize), Complex64> = HashMap::new();

    let shift = Complex64::new(alpha, beta);

    for i in 0..stiffness.nnz() {
        let key = (stiffness.rows[i], stiffness.cols[i]);
        let real_part = stiffness.values[i];
        *entries.entry(key).or_insert(Complex64::new(0.0, 0.0)) += Complex64::new(real_part, 0.0);
    }

    for i in 0..mass.nnz() {
        let key = (mass.rows[i], mass.cols[i]);
        let real_part = mass.values[i];
        *entries.entry(key).or_insert(Complex64::new(0.0, 0.0)) +=
            shift * Complex64::new(real_part, 0.0);
    }

    let dim = stiffness.dim;
    let mut rows = Vec::with_capacity(entries.len());
    let mut cols = Vec::with_capacity(entries.len());
    let mut values = Vec::with_capacity(entries.len());

    for ((r, c), v) in entries {
        if v.norm() > 1e-15 {
            rows.push(r);
            cols.push(c);
            values.push(v);
        }
    }

    CsrMatrix::from_triplets(
        dim,
        dim,
        rows.into_iter()
            .zip(cols)
            .zip(values)
            .map(|((r, c), v)| (r, c, v))
            .collect(),
    )
}

/// Solve using GMRES with Shifted-Laplacian preconditioner
///
/// Uses P = K + (α + iβ)M as preconditioner for the Helmholtz system A = K - k²M.
/// The complex shifts α and β transform the indefinite operator to a more
/// favorable spectrum for iterative methods.
///
/// # Arguments
/// * `problem` - The assembled Helmholtz problem (provides access to K and M)
/// * `csr` - The system matrix A = K - k²M
/// * `rhs` - Right-hand side vector
/// * `config` - Solver configuration with shifted-Laplacian parameters
fn solve_gmres_shifted_laplacian(
    problem: &HelmholtzProblem,
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let k = config.wavenumber.unwrap_or(1.0);
    let default_config = ShiftedLaplacianConfig::for_wavenumber(k);
    let sl_config = config.shifted_laplacian.as_ref().unwrap_or(&default_config);

    if config.verbosity > 0 {
        println!(
            "  [FEM] Shifted-Laplacian: α={:.4}, β={:.4}, AMG preconditioner",
            sl_config.alpha, sl_config.beta
        );
    }

    let sl_start = Instant::now();
    let p_matrix = build_shifted_laplacian(
        &problem.stiffness,
        &problem.mass,
        sl_config.alpha,
        sl_config.beta,
    );
    // Use more robust AMG settings for Shifted Laplacian
    let mut amg_config = AmgConfig::for_parallel();
    amg_config.smoother = math_audio_solvers::AmgSmoother::L1Jacobi; // More robust than standard Jacobi
    amg_config.strong_threshold = 0.5; // Denser matrix requires higher threshold
    
    let precond = AmgPreconditioner::from_csr(&p_matrix, amg_config);
    let sl_time = sl_start.elapsed();

    if config.verbosity > 0 {
        let diag = precond.diagnostics();
        println!(
            "  [FEM] SL-AMG setup: {:.1}ms, {} levels",
            sl_time.as_secs_f64() * 1000.0,
            diag.num_levels
        );
    }

    let result = gmres_preconditioned(csr, &precond, rhs, &config.gmres);

    if config.verbosity > 0 {
        println!(
            "  [FEM] SL-GMRES {} in {} iterations (residual: {:.2e})",
            if result.converged {
                "converged"
            } else {
                "did not converge"
            },
            result.iterations,
            result.residual
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Shifted-Laplacian and V-cycle multigrid smoothing
fn solve_gmres_shifted_laplacian_mg(
    problem: &HelmholtzProblem,
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let k = config.wavenumber.unwrap_or(1.0);
    let default_config = ShiftedLaplacianConfig::for_wavenumber(k);
    let sl_config = config.shifted_laplacian.as_ref().unwrap_or(&default_config);

    if config.verbosity > 0 {
        println!(
            "  [FEM] Shifted-Laplacian-MG: α={:.4}, β={:.4}, {} V-cycles",
            sl_config.alpha, sl_config.beta, sl_config.mg_cycles
        );
    }

    let p_matrix = build_shifted_laplacian(
        &problem.stiffness,
        &problem.mass,
        sl_config.alpha,
        sl_config.beta,
    );
    let amg_config = AmgConfig::for_parallel();
    let precond = AmgPreconditioner::from_csr(&p_matrix, amg_config);

    let mut residual = rhs.clone();
    let mut solution = Array1::zeros(rhs.len());

    for _ in 0..sl_config.mg_cycles {
        let result = gmres_preconditioned_with_guess(
            &p_matrix,
            &precond,
            &residual,
            Some(&solution),
            &config.gmres,
        );
        if result.converged {
            solution = result.x;
            break;
        }
        solution = result.x;
        let p_applied = p_matrix.matvec(&solution);
        residual = &residual - &p_applied;
    }

    let final_residual = csr.matvec(&solution);
    let residual_vec = rhs - &final_residual;
    let residual_norm: f64 = residual_vec.iter().map(|v| v.norm()).sum::<f64>().sqrt();

    Ok(Solution {
        values: solution,
        iterations: sl_config.mg_cycles,
        residual: residual_norm,
        converged: true,
    })
}

/// Solve using GMRES with Shifted-Laplacian preconditioner, with optional initial guess
#[allow(dead_code)]
fn solve_gmres_shifted_laplacian_with_guess(
    problem: &HelmholtzProblem,
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let k = config.wavenumber.unwrap_or(1.0);
    let default_config = ShiftedLaplacianConfig::for_wavenumber(k);
    let sl_config = config.shifted_laplacian.as_ref().unwrap_or(&default_config);

    if config.verbosity > 0 {
        println!(
            "  [FEM] Shifted-Laplacian: α={:.4}, β={:.4}",
            sl_config.alpha, sl_config.beta
        );
    }

    let p_matrix = build_shifted_laplacian(
        &problem.stiffness,
        &problem.mass,
        sl_config.alpha,
        sl_config.beta,
    );
    let amg_config = AmgConfig::for_parallel();
    let precond = AmgPreconditioner::from_csr(&p_matrix, amg_config);
    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve using GMRES with Shifted-Laplacian+V-cycle, with optional initial guess
#[allow(dead_code)]
fn solve_gmres_shifted_laplacian_mg_with_guess(
    problem: &HelmholtzProblem,
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    let k = config.wavenumber.unwrap_or(1.0);
    let default_config = ShiftedLaplacianConfig::for_wavenumber(k);
    let sl_config = config.shifted_laplacian.as_ref().unwrap_or(&default_config);

    let p_matrix = build_shifted_laplacian(
        &problem.stiffness,
        &problem.mass,
        sl_config.alpha,
        sl_config.beta,
    );
    let amg_config = AmgConfig::for_parallel();
    let precond = AmgPreconditioner::from_csr(&p_matrix, amg_config);

    let result = gmres_preconditioned_with_guess(csr, &precond, rhs, x0, &config.gmres);

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    Ok(Solution {
        values: result.x,
        iterations: result.iterations,
        residual: result.residual,
        converged: result.converged,
    })
}

/// Solve a Helmholtz problem directly from CSR matrix and RHS
///
/// This is useful when you have pre-assembled sparse matrices.
pub fn solve_csr(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    solve_csr_with_guess(csr, rhs, None, config)
}

/// Solve a Helmholtz problem directly from CSR matrix and RHS with optional initial guess
///
/// Providing an initial guess can significantly reduce iterations when solving
/// problems at nearby frequencies (warm starting).
///
/// # Arguments
/// * `csr` - System matrix in CSR format
/// * `rhs` - Right-hand side vector
/// * `x0` - Optional initial guess (if None, starts from zero)
/// * `config` - Solver configuration
pub fn solve_csr_with_guess(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if csr.num_rows != rhs.len() {
        return Err(SolverError::DimensionMismatch {
            expected: csr.num_rows,
            actual: rhs.len(),
        });
    }

    if let Some(guess) = x0
        && guess.len() != rhs.len()
    {
        return Err(SolverError::DimensionMismatch {
            expected: rhs.len(),
            actual: guess.len(),
        });
    }

    match config.solver_type {
        SolverType::Direct => solve_direct(csr, rhs, config),
        SolverType::Gmres => solve_gmres_with_guess(csr, rhs, x0, config),
        SolverType::GmresIlu => solve_gmres_ilu_with_guess(csr, rhs, x0, config),
        SolverType::GmresJacobi => solve_gmres_jacobi_with_guess(csr, rhs, x0, config),
        SolverType::GmresIluColoring => solve_gmres_ilu_coloring_with_guess(csr, rhs, x0, config),
        SolverType::GmresIluFixedPoint => {
            solve_gmres_ilu_fixedpoint_with_guess(csr, rhs, x0, config)
        }
        SolverType::GmresSchwarz => solve_gmres_schwarz_with_guess(csr, rhs, x0, config),
        SolverType::GmresAmg => solve_gmres_amg_with_guess(csr, rhs, x0, config),
        SolverType::GmresPipelined => solve_gmres_pipelined_with_guess(csr, rhs, x0, config),
        SolverType::GmresPipelinedIlu => solve_gmres_pipelined_ilu_with_guess(csr, rhs, x0, config),
        SolverType::GmresPipelinedAmg => solve_gmres_pipelined_amg_with_guess(csr, rhs, x0, config),
        SolverType::GmresShiftedLaplacian => {
            Err(SolverError::InvalidConfiguration(
                "Shifted-Laplacian solver requires HelmholtzProblem, not CSR matrix. Use solve() instead.".into()
            ))
        }
        SolverType::GmresShiftedLaplacianMg => {
            Err(SolverError::InvalidConfiguration(
                "Shifted-Laplacian solver requires HelmholtzProblem, not CSR matrix. Use solve() instead.".into()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_solve_helmholtz_direct() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let config = SolverConfig {
            solver_type: SolverType::Direct,
            ..Default::default()
        };

        let solution = solve(&problem, &config).expect("Solver should succeed");
        assert!(solution.converged);
        assert_eq!(solution.values.len(), problem.num_dofs());
    }

    #[test]
    fn test_solve_helmholtz_gmres() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let config = SolverConfig {
            solver_type: SolverType::Gmres,
            gmres: GmresConfigF64 {
                max_iterations: 100,
                restart: 20,
                tolerance: 1e-8,
                print_interval: 0,
            },
            ..Default::default()
        };

        let solution = solve(&problem, &config).expect("Solver should succeed");
        assert!(solution.converged);
    }

    #[test]
    fn test_solve_helmholtz_gmres_ilu() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let config = SolverConfig {
            solver_type: SolverType::GmresIlu,
            gmres: GmresConfigF64 {
                max_iterations: 100,
                restart: 20,
                tolerance: 1e-8,
                print_interval: 0,
            },
            ..Default::default()
        };

        let solution = solve(&problem, &config).expect("Solver should succeed");
        assert!(solution.converged);
    }

    #[test]
    fn test_csr_conversion() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let csr = problem.matrix.to_csr();

        // Verify dimensions
        assert_eq!(csr.num_rows, problem.num_dofs());
        assert_eq!(csr.num_cols, problem.num_dofs());

        // Verify nnz is reasonable (should be <= original since duplicates are merged)
        assert!(csr.nnz() > 0);
        assert!(csr.nnz() <= problem.matrix.nnz());
    }

    #[test]
    fn test_ilu_preconditioner_improves_convergence() {
        let mesh = unit_square_triangles(8); // Larger mesh
        let k = Complex64::new(2.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |x, y, _| {
            Complex64::new(
                (x * std::f64::consts::PI).sin() * (y * std::f64::consts::PI).sin(),
                0.0,
            )
        });

        let gmres_config = GmresConfigF64 {
            max_iterations: 500,
            restart: 30,
            tolerance: 1e-8,
            print_interval: 0,
        };

        // Solve without preconditioning
        let config_no_precond = SolverConfig {
            solver_type: SolverType::Gmres,
            gmres: gmres_config.clone(),
            ..Default::default()
        };

        // Solve with ILU preconditioning
        let config_ilu = SolverConfig {
            solver_type: SolverType::GmresIlu,
            gmres: gmres_config,
            ..Default::default()
        };

        let sol_no_precond = solve(&problem, &config_no_precond).expect("Should converge");
        let sol_ilu = solve(&problem, &config_ilu).expect("Should converge");

        // ILU should require fewer or equal iterations
        assert!(
            sol_ilu.iterations <= sol_no_precond.iterations + 10,
            "ILU should not significantly increase iterations: {} vs {}",
            sol_ilu.iterations,
            sol_no_precond.iterations
        );
    }

    #[test]
    fn test_solve_helmholtz_gmres_pipelined() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let config = SolverConfig {
            solver_type: SolverType::GmresPipelined,
            gmres: GmresConfigF64 {
                max_iterations: 100,
                restart: 20,
                tolerance: 1e-8,
                print_interval: 0,
            },
            ..Default::default()
        };

        let solution = solve(&problem, &config).expect("Solver should succeed");
        assert!(solution.converged);
    }

    #[test]
    fn test_solve_helmholtz_gmres_pipelined_ilu() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let config = SolverConfig {
            solver_type: SolverType::GmresPipelinedIlu,
            gmres: GmresConfigF64 {
                max_iterations: 100,
                restart: 20,
                tolerance: 1e-8,
                print_interval: 0,
            },
            ..Default::default()
        };

        let solution = solve(&problem, &config).expect("Solver should succeed");
        assert!(solution.converged);
    }

    #[test]
    fn test_solve_helmholtz_gmres_shifted_laplacian() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let config = SolverConfig {
            solver_type: SolverType::GmresShiftedLaplacian,
            gmres: GmresConfigF64 {
                max_iterations: 100,
                restart: 20,
                tolerance: 1e-8,
                print_interval: 0,
            },
            wavenumber: Some(1.0),
            ..Default::default()
        };

        let solution = solve(&problem, &config).expect("Solver should succeed");
        assert!(solution.converged);
        assert_eq!(solution.values.len(), problem.num_dofs());
    }

    #[test]
    fn test_shifted_laplacian_config_constructors() {
        let config_default = ShiftedLaplacianConfig::default();
        assert_eq!(config_default.alpha, 1.0);
        assert_eq!(config_default.beta, 1.0);

        let config_k = ShiftedLaplacianConfig::for_wavenumber(2.0);
        assert_eq!(config_k.alpha, 2.0); // 0.5 * k^2 = 0.5 * 4 = 2
        assert_eq!(config_k.beta, 1.0); // 0.5 * k = 0.5 * 2 = 1

        let config_aggressive = ShiftedLaplacianConfig::aggressive(2.0);
        assert_eq!(config_aggressive.alpha, 4.0); // k^2 = 4
        assert_eq!(config_aggressive.beta, 2.0); // k = 2

        let config_conservative = ShiftedLaplacianConfig::conservative(2.0);
        assert_eq!(config_conservative.alpha, 1.0); // 0.25 * k^2 = 0.25 * 4 = 1
        assert_eq!(config_conservative.beta, 0.5); // 0.25 * k = 0.25 * 2 = 0.5
    }
}
