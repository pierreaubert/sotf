//! GMRES (Generalized Minimal Residual) solver
//!
//! Implementation of the restarted GMRES algorithm based on Saad & Schultz (1986).
//!
//! GMRES is often the best choice for large non-symmetric systems like BEM.
//! It minimizes the residual in a Krylov subspace and has smooth, monotonic
//! convergence behavior.
//!
//! ## Algorithm
//!
//! GMRES builds an orthonormal basis for the Krylov subspace K_m = span{r, Ar, A²r, ...}
//! using the Arnoldi process, then finds the solution that minimizes ||b - Ax|| in this
//! subspace using a QR factorization approach.
//!
//! ## Restart
//!
//! Full GMRES requires storing m vectors where m is the number of iterations.
//! For large problems, this becomes prohibitive. Restarted GMRES(m) restarts
//! after m iterations to limit memory usage.
//!
//! Typical values:
//! - m = 20-50 for moderate problems
//! - m = 50-100 for larger problems
//! - m = 100-200 for very large BEM problems

use ndarray::{Array1, Array2};
use num_complex::Complex64;

/// GMRES solver configuration
#[derive(Debug, Clone)]
pub struct GmresConfig {
    /// Maximum number of outer iterations (restarts)
    pub max_iterations: usize,
    /// Restart parameter (number of inner iterations before restart)
    /// Also known as the Krylov subspace dimension
    pub restart: usize,
    /// Relative tolerance for convergence
    pub tolerance: f64,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for GmresConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            restart: 30, // GMRES(30) - good default for BEM
            tolerance: 1e-6,
            print_interval: 10,
        }
    }
}

impl GmresConfig {
    /// Create config for small problems (uses more memory, faster convergence)
    pub fn for_small_problems() -> Self {
        Self {
            max_iterations: 50,
            restart: 50,
            tolerance: 1e-8,
            print_interval: 0,
        }
    }

    /// Create config for large BEM problems
    pub fn for_large_bem() -> Self {
        Self {
            max_iterations: 200,
            restart: 100,
            tolerance: 1e-6,
            print_interval: 20,
        }
    }

    /// Create config with specific restart parameter
    pub fn with_restart(restart: usize) -> Self {
        Self {
            restart,
            ..Default::default()
        }
    }
}

/// GMRES solver result
#[derive(Debug)]
pub struct GmresSolution {
    /// Solution vector
    pub x: Array1<Complex64>,
    /// Total number of matrix-vector products
    pub iterations: usize,
    /// Number of restarts performed
    pub restarts: usize,
    /// Final relative residual
    pub residual: f64,
    /// Whether convergence was achieved
    pub converged: bool,
}

/// Solve Ax = b using the restarted GMRES method
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
/// let config = GmresConfig::with_restart(50);
/// let matvec = |x: &Array1<Complex64>| system.matvec(x);
/// let solution = gmres_solve(&matvec, &rhs, None, &config);
/// ```
pub fn gmres_solve<F>(
    matvec: F,
    b: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &GmresConfig,
) -> GmresSolution
where
    F: Fn(&Array1<Complex64>) -> Array1<Complex64>,
{
    let n = b.len();
    let m = config.restart;

    // Initialize solution vector
    let mut x = match x0 {
        Some(x0) => x0.clone(),
        None => Array1::zeros(n),
    };

    // Compute initial residual norm for relative tolerance
    let b_norm = vector_norm(b);
    if b_norm < 1e-15 {
        return GmresSolution {
            x,
            iterations: 0,
            restarts: 0,
            residual: 0.0,
            converged: true,
        };
    }

    let mut total_iterations = 0;
    let mut restarts = 0;

    // Outer iteration (restarts)
    for _outer in 0..config.max_iterations {
        // Compute residual r = b - Ax
        let ax = matvec(&x);
        let r: Array1<Complex64> = b - &ax;
        let beta = vector_norm(&r);

        // Check convergence
        let rel_residual = beta / b_norm;
        if rel_residual < config.tolerance {
            return GmresSolution {
                x,
                iterations: total_iterations,
                restarts,
                residual: rel_residual,
                converged: true,
            };
        }

        // Initialize Krylov basis V (n x (m+1))
        // V[:,0] = r / ||r||
        let mut v: Vec<Array1<Complex64>> = Vec::with_capacity(m + 1);
        v.push(&r / Complex64::new(beta, 0.0));

        // Upper Hessenberg matrix H ((m+1) x m)
        let mut h: Array2<Complex64> = Array2::zeros((m + 1, m));

        // Givens rotation coefficients
        let mut cs: Vec<Complex64> = Vec::with_capacity(m);
        let mut sn: Vec<Complex64> = Vec::with_capacity(m);

        // Right-hand side of least squares problem
        let mut g: Array1<Complex64> = Array1::zeros(m + 1);
        g[0] = Complex64::new(beta, 0.0);

        let mut inner_converged = false;

        // Inner iteration (Arnoldi process)
        for j in 0..m {
            total_iterations += 1;

            // w = A * v_j
            let w = matvec(&v[j]);
            let mut w = w;

            // Modified Gram-Schmidt orthogonalization
            for i in 0..=j {
                h[[i, j]] = inner_product(&v[i], &w);
                w = &w - &(&v[i] * h[[i, j]]);
            }

            h[[j + 1, j]] = Complex64::new(vector_norm(&w), 0.0);

            // Check for breakdown (lucky convergence or numerical issues)
            if h[[j + 1, j]].norm() < 1e-14 {
                // We can still get a solution from the current subspace
                inner_converged = true;
            } else {
                // Normalize and add to basis
                v.push(&w / h[[j + 1, j]]);
            }

            // Apply previous Givens rotations to new column of H
            for i in 0..j {
                let temp = cs[i].conj() * h[[i, j]] + sn[i].conj() * h[[i + 1, j]];
                h[[i + 1, j]] = -sn[i] * h[[i, j]] + cs[i] * h[[i + 1, j]];
                h[[i, j]] = temp;
            }

            // Compute new Givens rotation
            let (c, s) = givens_rotation(h[[j, j]], h[[j + 1, j]]);
            cs.push(c);
            sn.push(s);

            // Apply Givens rotation to H and g
            h[[j, j]] = c.conj() * h[[j, j]] + s.conj() * h[[j + 1, j]];
            h[[j + 1, j]] = Complex64::new(0.0, 0.0);

            let temp = c.conj() * g[j] + s.conj() * g[j + 1];
            g[j + 1] = -s * g[j] + c * g[j + 1];
            g[j] = temp;

            // Check convergence
            let rel_residual = g[j + 1].norm() / b_norm;

            if config.print_interval > 0 && total_iterations % config.print_interval == 0 {
                eprintln!(
                    "GMRES iteration {} (restart {}): relative residual = {:.6e}",
                    total_iterations, restarts, rel_residual
                );
            }

            if rel_residual < config.tolerance || inner_converged {
                // Solve upper triangular system Hy = g
                let y = solve_upper_triangular(&h, &g, j + 1);

                // Update solution x = x + V * y
                for (i, yi) in y.iter().enumerate() {
                    x = &x + &(&v[i] * *yi);
                }

                return GmresSolution {
                    x,
                    iterations: total_iterations,
                    restarts,
                    residual: rel_residual,
                    converged: true,
                };
            }
        }

        // Maximum inner iterations reached, compute solution and restart
        let y = solve_upper_triangular(&h, &g, m);

        // Update solution x = x + V * y
        for (i, yi) in y.iter().enumerate() {
            x = &x + &(&v[i] * *yi);
        }

        restarts += 1;
    }

    // Compute final residual
    let ax = matvec(&x);
    let r: Array1<Complex64> = b - &ax;
    let rel_residual = vector_norm(&r) / b_norm;

    GmresSolution {
        x,
        iterations: total_iterations,
        restarts,
        residual: rel_residual,
        converged: false,
    }
}

/// GMRES solver with preconditioner
///
/// Solves Ax = b using left preconditioning: M⁻¹Ax = M⁻¹b
pub fn gmres_solve_preconditioned<F, P>(
    matvec: F,
    precond_solve: P,
    b: &Array1<Complex64>,
    x0: Option<&Array1<Complex64>>,
    config: &GmresConfig,
) -> GmresSolution
where
    F: Fn(&Array1<Complex64>) -> Array1<Complex64>,
    P: Fn(&Array1<Complex64>) -> Array1<Complex64>,
{
    let n = b.len();
    let m = config.restart;

    // Initialize solution vector
    let mut x = match x0 {
        Some(x0) => x0.clone(),
        None => Array1::zeros(n),
    };

    // Compute preconditioned RHS norm
    let pb = precond_solve(b);
    let b_norm = vector_norm(&pb);
    if b_norm < 1e-15 {
        return GmresSolution {
            x,
            iterations: 0,
            restarts: 0,
            residual: 0.0,
            converged: true,
        };
    }

    let mut total_iterations = 0;
    let mut restarts = 0;

    for _outer in 0..config.max_iterations {
        // Compute preconditioned residual r = M⁻¹(b - Ax)
        let ax = matvec(&x);
        let residual: Array1<Complex64> = b - &ax;
        let r = precond_solve(&residual);
        let beta = vector_norm(&r);

        let rel_residual = beta / b_norm;
        if rel_residual < config.tolerance {
            return GmresSolution {
                x,
                iterations: total_iterations,
                restarts,
                residual: rel_residual,
                converged: true,
            };
        }

        let mut v: Vec<Array1<Complex64>> = Vec::with_capacity(m + 1);
        v.push(&r / Complex64::new(beta, 0.0));

        let mut h: Array2<Complex64> = Array2::zeros((m + 1, m));
        let mut cs: Vec<Complex64> = Vec::with_capacity(m);
        let mut sn: Vec<Complex64> = Vec::with_capacity(m);

        let mut g: Array1<Complex64> = Array1::zeros(m + 1);
        g[0] = Complex64::new(beta, 0.0);

        let mut inner_converged = false;

        for j in 0..m {
            total_iterations += 1;

            // w = M⁻¹ * A * v_j
            let av = matvec(&v[j]);
            let w = precond_solve(&av);
            let mut w = w;

            // Modified Gram-Schmidt
            for i in 0..=j {
                h[[i, j]] = inner_product(&v[i], &w);
                w = &w - &(&v[i] * h[[i, j]]);
            }

            h[[j + 1, j]] = Complex64::new(vector_norm(&w), 0.0);

            if h[[j + 1, j]].norm() < 1e-14 {
                inner_converged = true;
            } else {
                v.push(&w / h[[j + 1, j]]);
            }

            // Apply previous Givens rotations
            for i in 0..j {
                let temp = cs[i].conj() * h[[i, j]] + sn[i].conj() * h[[i + 1, j]];
                h[[i + 1, j]] = -sn[i] * h[[i, j]] + cs[i] * h[[i + 1, j]];
                h[[i, j]] = temp;
            }

            // Compute new Givens rotation
            let (c, s) = givens_rotation(h[[j, j]], h[[j + 1, j]]);
            cs.push(c);
            sn.push(s);

            // Apply to H and g
            h[[j, j]] = c.conj() * h[[j, j]] + s.conj() * h[[j + 1, j]];
            h[[j + 1, j]] = Complex64::new(0.0, 0.0);

            let temp = c.conj() * g[j] + s.conj() * g[j + 1];
            g[j + 1] = -s * g[j] + c * g[j + 1];
            g[j] = temp;

            let rel_residual = g[j + 1].norm() / b_norm;

            if config.print_interval > 0 && total_iterations % config.print_interval == 0 {
                eprintln!(
                    "GMRES (precond) iteration {} (restart {}): relative residual = {:.6e}",
                    total_iterations, restarts, rel_residual
                );
            }

            if rel_residual < config.tolerance || inner_converged {
                let y = solve_upper_triangular(&h, &g, j + 1);

                for (i, yi) in y.iter().enumerate() {
                    x = &x + &(&v[i] * *yi);
                }

                return GmresSolution {
                    x,
                    iterations: total_iterations,
                    restarts,
                    residual: rel_residual,
                    converged: true,
                };
            }
        }

        // Restart
        let y = solve_upper_triangular(&h, &g, m);
        for (i, yi) in y.iter().enumerate() {
            x = &x + &(&v[i] * *yi);
        }

        restarts += 1;
    }

    // Final residual
    let ax = matvec(&x);
    let residual: Array1<Complex64> = b - &ax;
    let r = precond_solve(&residual);
    let rel_residual = vector_norm(&r) / b_norm;

    GmresSolution {
        x,
        iterations: total_iterations,
        restarts,
        residual: rel_residual,
        converged: false,
    }
}

/// Compute inner product (x, y) = Σ conj(x_i) * y_i
#[inline]
fn inner_product(x: &Array1<Complex64>, y: &Array1<Complex64>) -> Complex64 {
    x.iter().zip(y.iter()).map(|(xi, yi)| xi.conj() * yi).sum()
}

/// Compute vector 2-norm
#[inline]
fn vector_norm(x: &Array1<Complex64>) -> f64 {
    x.iter().map(|xi| xi.norm_sqr()).sum::<f64>().sqrt()
}

/// Compute Givens rotation coefficients
///
/// Returns (c, s) such that:
/// [c*  s*] [a]   [r]
/// [-s  c ] [b] = [0]
#[inline]
fn givens_rotation(a: Complex64, b: Complex64) -> (Complex64, Complex64) {
    if b.norm() < 1e-30 {
        return (Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0));
    }
    if a.norm() < 1e-30 {
        return (Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0));
    }

    let r = (a.norm_sqr() + b.norm_sqr()).sqrt();
    let c = a / Complex64::new(r, 0.0);
    let s = b / Complex64::new(r, 0.0);

    (c, s)
}

/// Solve upper triangular system Hy = g
///
/// Only uses the upper k×k portion of H
fn solve_upper_triangular(
    h: &Array2<Complex64>,
    g: &Array1<Complex64>,
    k: usize,
) -> Vec<Complex64> {
    let mut y = vec![Complex64::new(0.0, 0.0); k];

    for i in (0..k).rev() {
        let mut sum = g[i];
        for j in (i + 1)..k {
            sum -= h[[i, j]] * y[j];
        }
        if h[[i, i]].norm() > 1e-30 {
            y[i] = sum / h[[i, i]];
        }
    }

    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gmres_simple() {
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

        let config = GmresConfig {
            max_iterations: 100,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres_solve(&matvec, &b, None, &config);

        assert!(solution.converged, "GMRES should converge");

        // Verify solution: Ax ≈ b
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_gmres_complex() {
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

        let config = GmresConfig {
            max_iterations: 100,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres_solve(&matvec, &b, None, &config);

        assert!(
            solution.converged,
            "GMRES should converge for complex system"
        );

        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_gmres_identity() {
        // Identity matrix - should converge in 1 iteration
        let n = 5;
        let b = Array1::from_vec(
            (1..=n)
                .map(|i| Complex64::new(i as f64, 0.0))
                .collect::<Vec<_>>(),
        );

        let matvec = |x: &Array1<Complex64>| x.clone();

        let config = GmresConfig {
            max_iterations: 10,
            restart: 10,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let solution = gmres_solve(&matvec, &b, None, &config);

        assert!(solution.converged);
        assert!(solution.iterations <= 2);

        let error: f64 = (&solution.x - &b)
            .iter()
            .map(|e| e.norm_sqr())
            .sum::<f64>()
            .sqrt();
        assert!(error < 1e-10);
    }

    #[test]
    fn test_gmres_non_symmetric() {
        // Non-symmetric matrix - GMRES should handle this
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

        let config = GmresConfig {
            max_iterations: 100,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres_solve(&matvec, &b, None, &config);

        assert!(
            solution.converged,
            "GMRES should converge for non-symmetric system"
        );

        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_gmres_restart() {
        // Larger system to test restart behavior
        let n = 20;
        let mut a = Array2::zeros((n, n));

        // Tridiagonal matrix
        for i in 0..n {
            a[[i, i]] = Complex64::new(4.0, 0.0);
            if i > 0 {
                a[[i, i - 1]] = Complex64::new(-1.0, 0.1);
            }
            if i < n - 1 {
                a[[i, i + 1]] = Complex64::new(-1.0, -0.1);
            }
        }

        let b: Array1<Complex64> =
            Array1::from_iter((0..n).map(|i| Complex64::new((i as f64 * 0.3).sin(), 0.0)));

        let matvec = |x: &Array1<Complex64>| a.dot(x);

        // Use small restart to force restarts
        let config = GmresConfig {
            max_iterations: 50,
            restart: 5,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres_solve(&matvec, &b, None, &config);

        println!(
            "GMRES: {} iterations, {} restarts, residual = {:.6e}",
            solution.iterations, solution.restarts, solution.residual
        );

        assert!(
            solution.converged,
            "GMRES should converge even with restarts"
        );

        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        let rel_error = error / b.iter().map(|bi| bi.norm_sqr()).sum::<f64>().sqrt();
        assert!(rel_error < 1e-8, "Solution should be accurate");
    }

    #[test]
    fn test_gmres_config_builders() {
        let small = GmresConfig::for_small_problems();
        assert_eq!(small.restart, 50);

        let large = GmresConfig::for_large_bem();
        assert_eq!(large.restart, 100);

        let custom = GmresConfig::with_restart(75);
        assert_eq!(custom.restart, 75);
    }
}
