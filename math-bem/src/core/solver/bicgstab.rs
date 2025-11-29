//! BiCGSTAB (Bi-Conjugate Gradient Stabilized) solver
//!
//! Implementation based on van der Vorst (1992).
//!
//! BiCGSTAB is often more stable than CGS for non-symmetric systems,
//! using GMRES-like smoothing to reduce irregular convergence behavior.
//!
//! ## Algorithm
//!
//! BiCGSTAB improves upon CGS by adding a stabilization step that
//! minimizes the residual in a 1D subspace at each iteration.

use ndarray::Array1;
use num_complex::Complex64;

/// BiCGSTAB solver configuration
#[derive(Debug, Clone)]
pub struct BiCgstabConfig {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Relative tolerance for convergence
    pub tolerance: f64,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for BiCgstabConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            print_interval: 10,
        }
    }
}

/// BiCGSTAB solver result
#[derive(Debug)]
pub struct BiCgstabSolution {
    /// Solution vector
    pub x: Array1<Complex64>,
    /// Number of iterations performed
    pub iterations: usize,
    /// Final relative residual
    pub residual: f64,
    /// Whether convergence was achieved
    pub converged: bool,
}

/// Solve Ax = b using the BiCGSTAB method
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
/// let config = BiCgstabConfig::default();
/// let matvec = |x: &Array1<Complex64>| system.matvec(x);
/// let solution = bicgstab_solve(&matvec, &rhs, None, &config);
/// ```
pub fn bicgstab_solve<F>(
    matvec: F,
    b: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &BiCgstabConfig,
) -> BiCgstabSolution
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

    // Compute initial residual norm
    let err_ori = residual_norm(&r);
    if err_ori < 1e-15 {
        return BiCgstabSolution {
            x,
            iterations: 0,
            residual: 0.0,
            converged: true,
        };
    }

    // Initialize scalars
    let mut rho = Complex64::new(1.0, 0.0);
    let mut alpha = Complex64::new(1.0, 0.0);
    let mut omega = Complex64::new(1.0, 0.0);

    // Initialize vectors
    let mut v: Array1<Complex64> = Array1::zeros(n);
    let mut p: Array1<Complex64> = Array1::zeros(n);

    let mut iterations = 0;
    let mut err_rel = 1.0;

    for j in 0..config.max_iterations {
        iterations = j + 1;

        // ρ_j = (r̃₀, r_{j-1})
        let rho_new = inner_product(&r_tilde, &r);

        // Check for breakdown
        if rho_new.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }

        // β = (ρ_j / ρ_{j-1}) * (α / ω)
        let beta = (rho_new / rho) * (alpha / omega);

        // p_j = r_{j-1} + β * (p_{j-1} - ω * v_{j-1})
        let p_minus_omega_v: Array1<Complex64> = &p - &(&v * omega);
        p = &r + &(&p_minus_omega_v * beta);

        // v_j = A * p_j
        v = matvec(&p);

        // α = ρ_j / (r̃₀, v_j)
        let r_tilde_v = inner_product(&r_tilde, &v);
        if r_tilde_v.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        alpha = rho_new / r_tilde_v;

        // s = r_{j-1} - α * v_j
        let s: Array1<Complex64> = &r - &(&v * alpha);

        // Check if s is small enough for convergence
        let s_norm = residual_norm(&s);
        if s_norm / err_ori < config.tolerance {
            // Update x and return
            x = &x + &(&p * alpha);
            return BiCgstabSolution {
                x,
                iterations,
                residual: s_norm / err_ori,
                converged: true,
            };
        }

        // t = A * s
        let t = matvec(&s);

        // ω = (t, s) / (t, t)
        let t_s = inner_product(&t, &s);
        let t_t = inner_product(&t, &t);
        if t_t.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        omega = t_s / t_t;

        // x_j = x_{j-1} + α * p_j + ω * s
        x = &x + &(&p * alpha) + &(&s * omega);

        // r_j = s - ω * t
        r = &s - &(&t * omega);

        // Compute residual norm
        let r_norm = residual_norm(&r);
        err_rel = r_norm / err_ori;

        // Print progress
        if config.print_interval > 0 && j % config.print_interval == 0 {
            eprintln!(
                "BiCGSTAB iteration {}: relative residual = {:.6e}",
                j, err_rel
            );
        }

        // Check convergence
        if err_rel < config.tolerance {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: true,
            };
        }

        // Check for breakdown in omega
        if omega.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }

        rho = rho_new;
    }

    BiCgstabSolution {
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

/// BiCGSTAB solver with preconditioner
///
/// Solves Ax = b using left preconditioning: M⁻¹Ax = M⁻¹b
pub fn bicgstab_solve_preconditioned<F, P>(
    matvec: F,
    precond_solve: P,
    b: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &BiCgstabConfig,
) -> BiCgstabSolution
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

    // Apply preconditioner
    let mut r = precond_solve(&r0);
    let r_tilde = r.clone();

    let err_ori = residual_norm(&r);
    if err_ori < 1e-15 {
        return BiCgstabSolution {
            x,
            iterations: 0,
            residual: 0.0,
            converged: true,
        };
    }

    let mut rho = Complex64::new(1.0, 0.0);
    let mut alpha = Complex64::new(1.0, 0.0);
    let mut omega = Complex64::new(1.0, 0.0);

    let mut v: Array1<Complex64> = Array1::zeros(n);
    let mut p: Array1<Complex64> = Array1::zeros(n);

    let mut iterations = 0;
    let mut err_rel = 1.0;

    for j in 0..config.max_iterations {
        iterations = j + 1;

        let rho_new = inner_product(&r_tilde, &r);
        if rho_new.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }

        let beta = (rho_new / rho) * (alpha / omega);
        let p_minus_omega_v: Array1<Complex64> = &p - &(&v * omega);
        p = &r + &(&p_minus_omega_v * beta);

        // v = M⁻¹ * A * p
        let ap = matvec(&p);
        v = precond_solve(&ap);

        let r_tilde_v = inner_product(&r_tilde, &v);
        if r_tilde_v.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        alpha = rho_new / r_tilde_v;

        let s: Array1<Complex64> = &r - &(&v * alpha);
        let s_norm = residual_norm(&s);
        if s_norm / err_ori < config.tolerance {
            x = &x + &(&p * alpha);
            return BiCgstabSolution {
                x,
                iterations,
                residual: s_norm / err_ori,
                converged: true,
            };
        }

        // t = M⁻¹ * A * s
        let as_ = matvec(&s);
        let t = precond_solve(&as_);

        let t_s = inner_product(&t, &s);
        let t_t = inner_product(&t, &t);
        if t_t.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }
        omega = t_s / t_t;

        x = &x + &(&p * alpha) + &(&s * omega);
        r = &s - &(&t * omega);

        let r_norm = residual_norm(&r);
        err_rel = r_norm / err_ori;

        if config.print_interval > 0 && j % config.print_interval == 0 {
            eprintln!(
                "BiCGSTAB (precond) iteration {}: relative residual = {:.6e}",
                j, err_rel
            );
        }

        if err_rel < config.tolerance {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: true,
            };
        }

        if omega.norm() < 1e-30 {
            return BiCgstabSolution {
                x,
                iterations,
                residual: err_rel,
                converged: false,
            };
        }

        rho = rho_new;
    }

    BiCgstabSolution {
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
    fn test_bicgstab_simple() {
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

        let config = BiCgstabConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = bicgstab_solve(&matvec, &b, None, &config);

        assert!(solution.converged, "BiCGSTAB should converge");

        // Verify solution: Ax ≈ b
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_bicgstab_complex() {
        // Complex non-symmetric system
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

        let config = BiCgstabConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = bicgstab_solve(&matvec, &b, None, &config);

        assert!(
            solution.converged,
            "BiCGSTAB should converge for complex system"
        );

        // Verify solution
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_bicgstab_identity() {
        // Identity matrix - should converge in 1 iteration
        let n = 5;
        let b = Array1::from_vec(
            (1..=n)
                .map(|i| Complex64::new(i as f64, 0.0))
                .collect::<Vec<_>>(),
        );

        let matvec = |x: &Array1<Complex64>| x.clone();

        let config = BiCgstabConfig {
            max_iterations: 10,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let solution = bicgstab_solve(&matvec, &b, None, &config);

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
    fn test_bicgstab_non_symmetric() {
        // Non-symmetric matrix - BiCGSTAB should handle this
        let a = Array2::from_shape_vec(
            (3, 3),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(5.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(3.0, 0.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![
            Complex64::new(5.0, 0.0),
            Complex64::new(8.0, 0.0),
            Complex64::new(4.0, 0.0),
        ]);

        let matvec = |x: &Array1<Complex64>| a.dot(x);

        let config = BiCgstabConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = bicgstab_solve(&matvec, &b, None, &config);

        assert!(
            solution.converged,
            "BiCGSTAB should converge for non-symmetric system"
        );

        // Verify solution
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_bicgstab_vs_cgs_stability() {
        // System that might be challenging for CGS but stable for BiCGSTAB
        let a = Array2::from_shape_vec(
            (3, 3),
            vec![
                Complex64::new(1.0, 0.1),
                Complex64::new(0.5, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.5, 0.0),
                Complex64::new(1.0, -0.1),
                Complex64::new(0.5, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.5, 0.0),
                Complex64::new(1.0, 0.1),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]);

        let matvec = |x: &Array1<Complex64>| a.dot(x);

        let config = BiCgstabConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = bicgstab_solve(&matvec, &b, None, &config);

        assert!(solution.converged);

        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8);
    }
}
