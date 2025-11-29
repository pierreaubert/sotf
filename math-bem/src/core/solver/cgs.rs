//! Conjugate Gradient Squared (CGS) solver
//!
//! Direct port of NC_IterativeSolverCGS from NC_CommonFunctions.cpp.
//!
//! The CGS algorithm is based on A. Meister p.168 and provides faster
//! convergence than standard CG for non-symmetric systems.
//!
//! ## Algorithm
//!
//! CGS squares the convergence polynomial of BiCG without needing
//! the transpose matrix. This makes it suitable for BEM systems.

use ndarray::Array1;
use num_complex::Complex64;

/// CGS solver configuration
#[derive(Debug, Clone)]
pub struct CgsConfig {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Relative tolerance for convergence
    pub tolerance: f64,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for CgsConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            print_interval: 10,
        }
    }
}

/// CGS solver result
#[derive(Debug)]
pub struct CgsSolution {
    /// Solution vector
    pub x: Array1<Complex64>,
    /// Number of iterations performed
    pub iterations: usize,
    /// Final relative residual
    pub residual: f64,
    /// Whether convergence was achieved
    pub converged: bool,
}

/// Solve Ax = b using the Conjugate Gradient Squared method
///
/// # Arguments
/// * `matvec` - Function to compute A*x for a given x
/// * `b` - Right-hand side vector
/// * `x0` - Optional initial guess (defaults to zero)
/// * `config` - Solver configuration
///
/// # Returns
/// Solution struct containing x, iteration count, and convergence info
///
/// # Example
/// ```ignore
/// let config = CgsConfig::default();
/// let matvec = |x: &Array1<Complex64>| system.matvec(x);
/// let solution = cgs_solve(&matvec, &rhs, None, &config);
/// ```
pub fn cgs_solve<F>(
    matvec: F,
    b: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &CgsConfig,
) -> CgsSolution
where
    F: Fn(&Array1<Complex64>) -> Array1<Complex64>,
{
    let n = b.len();

    // Initialize solution vector
    let mut x = match x0 {
        Some(x0) => x0.clone(),
        None => Array1::zeros(n),
    };

    // Initial residual: r = b - A*x
    let ax = matvec(&x);
    let mut r: Array1<Complex64> = b - &ax;

    // r̃₀ = r₀ (shadow residual, kept constant)
    let r_tilde = r.clone();

    // Initialize vectors
    let mut u = r.clone();
    let mut p = r.clone();

    // Compute initial residual norm
    let err_ori = residual_norm(&r);
    if err_ori < 1e-15 {
        return CgsSolution {
            x,
            iterations: 0,
            residual: 0.0,
            converged: true,
        };
    }

    // (r₀, r̃₀)
    let mut rho = inner_product(&r, &r_tilde);

    let mut iterations = 0;
    let mut err_rel = 1.0;

    for j in 0..config.max_iterations {
        iterations = j + 1;

        // v = A * p
        let v = matvec(&p);

        // α = (r_j, r̃₀) / (v, r̃₀)
        let v_r_tilde = inner_product(&v, &r_tilde);
        if v_r_tilde.norm() < 1e-30 {
            // Breakdown - return current solution
            return CgsSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        let alpha = rho / v_r_tilde;

        // q = u - α*v
        let q: Array1<Complex64> = &u - &(&v * alpha);

        // u_q = u + q
        let u_q: Array1<Complex64> = &u + &q;

        // A*(u + q)
        let a_uq = matvec(&u_q);

        // x_{j+1} = x_j + α*(u + q)
        x = &x + &(&u_q * alpha);

        // r_{j+1} = r_j - α*A*(u + q)
        r = &r - &(&a_uq * alpha);

        // Compute residual norm
        let r_norm = residual_norm(&r);
        err_rel = r_norm / err_ori;

        // Print progress
        if config.print_interval > 0 && j % config.print_interval == 0 {
            eprintln!("CGS iteration {}: relative residual = {:.6e}", j, err_rel);
        }

        // Check convergence
        if err_rel < config.tolerance {
            return CgsSolution {
                x,
                iterations,
                residual: err_rel,
                converged: true,
            };
        }

        // (r_{j+1}, r̃₀)
        let rho_new = inner_product(&r, &r_tilde);

        // β = (r_{j+1}, r̃₀) / (r_j, r̃₀)
        if rho.norm() < 1e-30 {
            // Breakdown
            return CgsSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        let beta = rho_new / rho;

        // u_{j+1} = r_{j+1} + β*q
        u = &r + &(&q * beta);

        // p_{j+1} = u_{j+1} + β*(q + β*p_j)
        let q_beta_p: Array1<Complex64> = &q + &(&p * beta);
        p = &u + &(&q_beta_p * beta);

        rho = rho_new;
    }

    CgsSolution {
        x,
        iterations,
        residual: err_rel,
        converged: false,
    }
}

/// Compute inner product (x, y) = Σ conj(x_i) * y_i
fn inner_product(x: &Array1<Complex64>, y: &Array1<Complex64>) -> Complex64 {
    x.iter().zip(y.iter()).map(|(xi, yi)| xi.conj() * yi).sum()
}

/// Compute residual norm ||r||₂
fn residual_norm(r: &Array1<Complex64>) -> f64 {
    r.iter().map(|ri| ri.norm_sqr()).sum::<f64>().sqrt()
}

/// CGS solver with preconditioner
///
/// Solves M⁻¹Ax = M⁻¹b where M is the preconditioner
pub fn cgs_solve_preconditioned<F, P>(
    matvec: F,
    precond_solve: P,
    b: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &CgsConfig,
) -> CgsSolution
where
    F: Fn(&Array1<Complex64>) -> Array1<Complex64>,
    P: Fn(&Array1<Complex64>) -> Array1<Complex64>,
{
    let n = b.len();

    // Initialize solution vector
    let mut x = match x0 {
        Some(x0) => x0.clone(),
        None => Array1::zeros(n),
    };

    // Initial residual: r = b - A*x
    let ax = matvec(&x);
    let r0: Array1<Complex64> = b - &ax;

    // Apply preconditioner to initial residual
    let mut r = precond_solve(&r0);

    // r̃₀ = r₀ (shadow residual)
    let r_tilde = r.clone();

    // Initialize vectors
    let mut u = r.clone();
    let mut p = r.clone();

    // Compute initial residual norm
    let err_ori = residual_norm(&r);
    if err_ori < 1e-15 {
        return CgsSolution {
            x,
            iterations: 0,
            residual: 0.0,
            converged: true,
        };
    }

    let mut rho = inner_product(&r, &r_tilde);

    let mut iterations = 0;
    let mut err_rel = 1.0;

    for j in 0..config.max_iterations {
        iterations = j + 1;

        // v = M⁻¹ * A * p
        let ap = matvec(&p);
        let v = precond_solve(&ap);

        // α = (r_j, r̃₀) / (v, r̃₀)
        let v_r_tilde = inner_product(&v, &r_tilde);
        if v_r_tilde.norm() < 1e-30 {
            return CgsSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        let alpha = rho / v_r_tilde;

        // q = u - α*v
        let q: Array1<Complex64> = &u - &(&v * alpha);

        // u_q = u + q
        let u_q: Array1<Complex64> = &u + &q;

        // A*(u + q)
        let a_uq = matvec(&u_q);
        let ma_uq = precond_solve(&a_uq);

        // x_{j+1} = x_j + α*(u + q)
        x = &x + &(&u_q * alpha);

        // r_{j+1} = r_j - α*M⁻¹*A*(u + q)
        r = &r - &(&ma_uq * alpha);

        // Compute residual norm
        let r_norm = residual_norm(&r);
        err_rel = r_norm / err_ori;

        if config.print_interval > 0 && j % config.print_interval == 0 {
            eprintln!(
                "CGS (precond) iteration {}: relative residual = {:.6e}",
                j, err_rel
            );
        }

        if err_rel < config.tolerance {
            return CgsSolution {
                x,
                iterations,
                residual: err_rel,
                converged: true,
            };
        }

        let rho_new = inner_product(&r, &r_tilde);
        if rho.norm() < 1e-30 {
            return CgsSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        let beta = rho_new / rho;

        u = &r + &(&q * beta);
        let q_beta_p: Array1<Complex64> = &q + &(&p * beta);
        p = &u + &(&q_beta_p * beta);

        rho = rho_new;
    }

    CgsSolution {
        x,
        iterations,
        residual: err_rel,
        converged: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_cgs_simple() {
        // Simple 2x2 positive definite system
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(3.0, 0.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)]);

        let matvec = |x: &Array1<Complex64>| a.dot(x);

        let config = CgsConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = cgs_solve(&matvec, &b, None, &config);

        assert!(solution.converged, "CGS should converge");

        // Verify solution: Ax ≈ b
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_cgs_complex() {
        // Complex system
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(2.0, 1.0),
                Complex64::new(0.0, -1.0),
                Complex64::new(0.0, 1.0),
                Complex64::new(2.0, -1.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![Complex64::new(1.0, 1.0), Complex64::new(1.0, -1.0)]);

        let matvec = |x: &Array1<Complex64>| a.dot(x);

        let config = CgsConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = cgs_solve(&matvec, &b, None, &config);

        assert!(solution.converged, "CGS should converge for complex system");

        // Verify solution
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_cgs_identity() {
        // Identity matrix - should converge in 1 iteration
        let n = 5;
        let b = Array1::from_vec(
            (1..=n)
                .map(|i| Complex64::new(i as f64, 0.0))
                .collect::<Vec<_>>(),
        );

        let matvec = |x: &Array1<Complex64>| x.clone();

        let config = CgsConfig {
            max_iterations: 10,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let solution = cgs_solve(&matvec, &b, None, &config);

        assert!(solution.converged);
        assert!(solution.iterations <= 2); // Should converge very quickly

        // x should equal b for identity matrix
        let error: f64 = (&solution.x - &b)
            .iter()
            .map(|e| e.norm_sqr())
            .sum::<f64>()
            .sqrt();
        assert!(error < 1e-10);
    }

    #[test]
    fn test_inner_product() {
        let x = Array1::from_vec(vec![Complex64::new(1.0, 2.0), Complex64::new(3.0, -1.0)]);
        let y = Array1::from_vec(vec![Complex64::new(2.0, 0.0), Complex64::new(0.0, 1.0)]);

        let result = inner_product(&x, &y);

        // (1-2i)*2 + (3+1i)*(1i) = 2-4i + 3i-1 = 1-i
        let expected = Complex64::new(1.0, -1.0);
        assert!((result - expected).norm() < 1e-10);
    }
}
