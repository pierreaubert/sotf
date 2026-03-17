//! CG (Conjugate Gradient) solver
//!
//! The Conjugate Gradient method for symmetric positive definite systems.
//! This is the method of choice for SPD matrices as it has optimal convergence.

use crate::blas_helpers::{inner_product, vector_norm};
use crate::traits::{ComplexField, LinearOperator, SolverStatus};
use ndarray::Array1;
use num_traits::{FromPrimitive, ToPrimitive, Zero};

/// CG solver configuration
#[derive(Debug, Clone)]
pub struct CgConfig<R> {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Relative tolerance for convergence
    pub tolerance: R,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for CgConfig<f64> {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            print_interval: 0,
        }
    }
}

/// CG solver result
#[derive(Debug)]
pub struct CgSolution<T: ComplexField> {
    /// Solution vector
    pub x: Array1<T>,
    /// Number of iterations
    pub iterations: usize,
    /// Final relative residual
    pub residual: T::Real,
    /// Whether convergence was achieved
    pub converged: bool,
    /// Solver status
    pub status: SolverStatus,
}

/// Solve Ax = b using the Conjugate Gradient method
///
/// Note: This method is only correct for symmetric positive definite matrices.
/// For non-symmetric systems, use GMRES or BiCGSTAB instead.
pub fn cg<T, A>(operator: &A, b: &Array1<T>, config: &CgConfig<T::Real>) -> CgSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
{
    let n = b.len();
    let mut x = Array1::from_elem(n, T::zero());

    let b_norm = vector_norm(b);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return CgSolution {
            x,
            iterations: 0,
            residual: T::Real::zero(),
            converged: true,
            status: SolverStatus::Converged,
        };
    }

    // Initial residual r = b - Ax = b (since x = 0)
    let mut r = b.clone();
    let mut p = r.clone();
    let mut rho = inner_product(&r, &r);

    let scaled = b_norm * T::Real::from_f64(1e-20).unwrap();
    let floor = T::Real::from_f64(1e-30).unwrap();
    let breakdown_threshold = if scaled > floor { scaled } else { floor };

    for iter in 0..config.max_iterations {
        // q = A * p
        let q = operator.apply(&p);

        // alpha = rho / (p, q)
        let pq = inner_product(&p, &q);
        if pq.norm() < breakdown_threshold {
            return CgSolution {
                x,
                iterations: iter,
                residual: vector_norm(&r) / b_norm,
                converged: false,
                status: SolverStatus::Breakdown,
            };
        }

        let alpha = rho / pq;

        // x = x + alpha * p
        x = &x + &p.mapv(|pi| pi * alpha);

        // r = r - alpha * q
        r = &r - &q.mapv(|qi| qi * alpha);

        let rel_residual = vector_norm(&r) / b_norm;

        if config.print_interval > 0 && (iter + 1) % config.print_interval == 0 {
            log::info!(
                "CG iteration {}: relative residual = {:.6e}",
                iter + 1,
                rel_residual.to_f64().unwrap_or(0.0)
            );
        }

        if rel_residual < config.tolerance {
            return CgSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: true,
                status: SolverStatus::Converged,
            };
        }

        let rho_new = inner_product(&r, &r);
        if rho_new.norm() < breakdown_threshold {
            return CgSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: false,
                status: SolverStatus::Breakdown,
            };
        }

        let beta = rho_new / rho;
        rho = rho_new;

        // p = r + beta * p
        p = &r + &p.mapv(|pi| pi * beta);
    }

    let rel_residual = vector_norm(&r) / b_norm;
    CgSolution {
        x,
        iterations: config.max_iterations,
        residual: rel_residual,
        converged: false,
        status: SolverStatus::MaxIterationsReached,
    }
}

/// Solve Ax = b using the Preconditioned Conjugate Gradient method
///
/// Uses a preconditioner M to accelerate convergence. At each iteration,
/// the preconditioner is applied as z = M^{-1} r, and the search direction
/// is computed using the preconditioned residual.
///
/// Note: Both A and M^{-1} must be symmetric positive definite.
pub fn pcg<T, A, P>(
    operator: &A,
    precond: &P,
    b: &Array1<T>,
    config: &CgConfig<T::Real>,
) -> CgSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
    P: crate::traits::Preconditioner<T>,
{
    let n = b.len();
    let mut x = Array1::from_elem(n, T::zero());

    let b_norm = vector_norm(b);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return CgSolution {
            x,
            iterations: 0,
            residual: T::Real::zero(),
            converged: true,
            status: SolverStatus::Converged,
        };
    }

    // Initial residual r = b - Ax = b (since x = 0)
    let mut r = b.clone();
    let mut z = precond.apply(&r);
    let mut p = z.clone();
    let mut rho = inner_product(&r, &z);

    let scaled = b_norm * T::Real::from_f64(1e-20).unwrap();
    let floor = T::Real::from_f64(1e-30).unwrap();
    let breakdown_threshold = if scaled > floor { scaled } else { floor };

    for iter in 0..config.max_iterations {
        let q = operator.apply(&p);

        let pq = inner_product(&p, &q);
        if pq.norm() < breakdown_threshold {
            return CgSolution {
                x,
                iterations: iter,
                residual: vector_norm(&r) / b_norm,
                converged: false,
                status: SolverStatus::Breakdown,
            };
        }

        let alpha = rho / pq;

        // x = x + alpha * p
        x = &x + &p.mapv(|pi| pi * alpha);

        // r = r - alpha * q
        r = &r - &q.mapv(|qi| qi * alpha);

        let rel_residual = vector_norm(&r) / b_norm;

        if config.print_interval > 0 && (iter + 1) % config.print_interval == 0 {
            log::info!(
                "PCG iteration {}: relative residual = {:.6e}",
                iter + 1,
                rel_residual.to_f64().unwrap_or(0.0)
            );
        }

        if rel_residual < config.tolerance {
            return CgSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: true,
                status: SolverStatus::Converged,
            };
        }

        z = precond.apply(&r);
        let rho_new = inner_product(&r, &z);
        if rho_new.norm() < breakdown_threshold {
            return CgSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: false,
                status: SolverStatus::Breakdown,
            };
        }

        let beta = rho_new / rho;
        rho = rho_new;

        // p = z + beta * p
        p = &z + &p.mapv(|pi| pi * beta);
    }

    let rel_residual = vector_norm(&r) / b_norm;
    CgSolution {
        x,
        iterations: config.max_iterations,
        residual: rel_residual,
        converged: false,
        status: SolverStatus::MaxIterationsReached,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparse::CsrMatrix;
    use ndarray::array;

    #[test]
    fn test_cg_spd() {
        // Symmetric positive definite matrix
        let dense = array![[4.0_f64, 1.0], [1.0, 3.0],];

        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![1.0_f64, 2.0];

        let config = CgConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = cg(&a, &b, &config);

        assert!(solution.converged, "CG should converge for SPD matrix");

        let ax = a.matvec(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e * e).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_cg_identity() {
        let n = 5;
        let id: CsrMatrix<f64> = CsrMatrix::identity(n);
        let b = Array1::from_iter((1..=n).map(|i| i as f64));

        let config = CgConfig {
            max_iterations: 10,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let solution = cg(&id, &b, &config);

        assert!(solution.converged);
        assert!(solution.iterations <= 2);

        let error: f64 = (&solution.x - &b).iter().map(|e| e * e).sum::<f64>().sqrt();
        assert!(error < 1e-10);
    }
}
