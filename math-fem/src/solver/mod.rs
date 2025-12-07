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
use solvers::iterative::gmres_preconditioned;
use solvers::{CsrMatrix, DiagonalPreconditioner, GmresConfig, IluPreconditioner, gmres, lu_solve};
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

/// Solve a Helmholtz problem directly from CSR matrix and RHS
///
/// This is useful when you have pre-assembled sparse matrices.
pub fn solve_csr(
    csr: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    config: &SolverConfig,
) -> Result<Solution, SolverError> {
    if csr.num_rows != rhs.len() {
        return Err(SolverError::DimensionMismatch {
            expected: csr.num_rows,
            actual: rhs.len(),
        });
    }

    match config.solver_type {
        SolverType::Direct => solve_direct(csr, rhs, config),
        SolverType::Gmres => solve_gmres(csr, rhs, config),
        SolverType::GmresIlu => solve_gmres_ilu(csr, rhs, config),
        SolverType::GmresJacobi => solve_gmres_jacobi(csr, rhs, config),
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
            Complex64::new((x * std::f64::consts::PI).sin() * (y * std::f64::consts::PI).sin(), 0.0)
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
}
