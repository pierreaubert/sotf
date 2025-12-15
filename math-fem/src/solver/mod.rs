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

use crate::assembly::HelmholtzProblem;
use ndarray::Array1;
use num_complex::Complex64;
use solvers::iterative::{gmres_pipelined, gmres_preconditioned, gmres_preconditioned_with_guess};
use solvers::{
    AdditiveSchwarzPreconditioner, AmgConfig, AmgPreconditioner, CsrMatrix, DiagonalPreconditioner,
    GmresConfig, IdentityPreconditioner, IluColoringPreconditioner, IluFixedPointPreconditioner,
    IluPreconditioner, gmres, lu_solve,
};
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
    };
    let solve_time = solve_start.elapsed();

    if config.verbosity > 0 {
        if let Ok(ref sol) = result {
            println!(
                "  [FEM] Solve: {} iters, residual {:.2e}, time {:.1}ms",
                sol.iterations,
                sol.residual,
                solve_time.as_secs_f64() * 1000.0
            );
        }
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
    // Use PMIS coarsening and Jacobi smoothing for best parallel performance
    let amg_config = AmgConfig::for_parallel();
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
    let amg_config = AmgConfig::for_parallel();
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

    if let Some(guess) = x0 {
        if guess.len() != rhs.len() {
            return Err(SolverError::DimensionMismatch {
                expected: rhs.len(),
                actual: guess.len(),
            });
        }
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
}
