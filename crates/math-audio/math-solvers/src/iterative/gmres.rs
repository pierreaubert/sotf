//! GMRES (Generalized Minimal Residual) solver
//!
//! Implementation of the restarted GMRES algorithm based on Saad & Schultz (1986).
//!
//! GMRES is often the best choice for large non-symmetric systems.
//! It minimizes the residual in a Krylov subspace and has smooth, monotonic
//! convergence behavior.

use crate::blas_helpers::{axpy, inner_product, vector_norm};
use crate::traits::{ComplexField, LinearOperator, Preconditioner};
use ndarray::{Array1, Array2};
use num_traits::{Float, FromPrimitive, One, ToPrimitive, Zero};

/// GMRES solver configuration
#[derive(Debug, Clone)]
pub struct GmresConfig<R> {
    /// Maximum number of outer iterations (restarts)
    pub max_iterations: usize,
    /// Restart parameter (number of inner iterations before restart)
    pub restart: usize,
    /// Relative tolerance for convergence
    pub tolerance: R,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for GmresConfig<f64> {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            restart: 30,
            tolerance: 1e-6,
            print_interval: 0,
        }
    }
}

impl Default for GmresConfig<f32> {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            restart: 30,
            tolerance: 1e-5,
            print_interval: 0,
        }
    }
}

impl<R: Float + FromPrimitive> GmresConfig<R> {
    /// Create config for small problems
    pub fn for_small_problems() -> Self {
        Self {
            max_iterations: 50,
            restart: 50,
            tolerance: R::from_f64(1e-8).unwrap(),
            print_interval: 0,
        }
    }

    /// Create config with specific restart parameter
    pub fn with_restart(restart: usize) -> Self
    where
        Self: Default,
    {
        Self {
            restart,
            ..Default::default()
        }
    }
}

/// GMRES solver result
#[derive(Debug)]
pub struct GmresSolution<T: ComplexField> {
    /// Solution vector
    pub x: Array1<T>,
    /// Total number of matrix-vector products
    pub iterations: usize,
    /// Number of restarts performed
    pub restarts: usize,
    /// Final relative residual
    pub residual: T::Real,
    /// Whether convergence was achieved
    pub converged: bool,
}

/// Solve Ax = b using the restarted GMRES method
///
/// # Arguments
/// * `operator` - Linear operator representing A
/// * `b` - Right-hand side vector
/// * `config` - Solver configuration
///
/// # Returns
/// Solution struct containing x, iteration count, and convergence info
pub fn gmres<T, A>(operator: &A, b: &Array1<T>, config: &GmresConfig<T::Real>) -> GmresSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
{
    gmres_with_guess(operator, b, None, config)
}

/// Solve Ax = b using GMRES with an initial guess
pub fn gmres_with_guess<T, A>(
    operator: &A,
    b: &Array1<T>,
    x0: Option<&Array1<T>>,
    config: &GmresConfig<T::Real>,
) -> GmresSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
{
    let n = b.len();
    let m = config.restart;

    // Initialize solution vector
    let mut x = match x0 {
        Some(x0) => x0.clone(),
        None => Array1::from_elem(n, T::zero()),
    };

    // Compute initial residual norm for relative tolerance
    let b_norm = vector_norm(b);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return GmresSolution {
            x,
            iterations: 0,
            restarts: 0,
            residual: T::Real::zero(),
            converged: true,
        };
    }

    let mut total_iterations = 0;
    let mut restarts = 0;

    // Outer iteration (restarts)
    for _outer in 0..config.max_iterations {
        // Compute residual r = b - Ax
        let ax = operator.apply(&x);
        let r: Array1<T> = b - &ax;
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

        // Initialize Krylov basis V
        let mut v: Vec<Array1<T>> = Vec::with_capacity(m + 1);
        v.push(r.mapv(|ri| ri * T::from_real(T::Real::one() / beta)));

        // Upper Hessenberg matrix H
        let mut h: Array2<T> = Array2::from_elem((m + 1, m), T::zero());

        // Givens rotation coefficients
        let mut cs: Vec<T> = Vec::with_capacity(m);
        let mut sn: Vec<T> = Vec::with_capacity(m);

        // Right-hand side of least squares problem
        let mut g: Array1<T> = Array1::from_elem(m + 1, T::zero());
        g[0] = T::from_real(beta);

        let mut inner_converged = false;

        // Inner iteration (Arnoldi process)
        for j in 0..m {
            total_iterations += 1;

            // w = A * v_j
            let mut w = operator.apply(&v[j]);

            // Modified Gram-Schmidt orthogonalization
            for i in 0..=j {
                h[[i, j]] = inner_product(&v[i], &w);
                let h_ij = h[[i, j]];
                axpy(-h_ij, &v[i], &mut w);
            }

            let w_norm = vector_norm(&w);
            h[[j + 1, j]] = T::from_real(w_norm);

            // Check for breakdown
            let breakdown_tol = T::Real::from_f64(1e-14).unwrap();
            if w_norm < breakdown_tol {
                inner_converged = true;
            } else {
                let inv_norm = T::from_real(T::Real::one() / w_norm);
                let mut new_v = w.clone();
                axpy(inv_norm - T::one(), &w, &mut new_v);
                v.push(new_v);
            }

            // Apply previous Givens rotations to new column of H
            for i in 0..j {
                let temp = cs[i].conj() * h[[i, j]] + sn[i].conj() * h[[i + 1, j]];
                h[[i + 1, j]] = T::zero() - sn[i] * h[[i, j]] + cs[i] * h[[i + 1, j]];
                h[[i, j]] = temp;
            }

            // Compute new Givens rotation
            let (c, s) = givens_rotation(h[[j, j]], h[[j + 1, j]]);
            cs.push(c);
            sn.push(s);

            // Apply Givens rotation to H and g
            h[[j, j]] = c.conj() * h[[j, j]] + s.conj() * h[[j + 1, j]];
            h[[j + 1, j]] = T::zero();

            let temp = c.conj() * g[j] + s.conj() * g[j + 1];
            g[j + 1] = T::zero() - s * g[j] + c * g[j + 1];
            g[j] = temp;

            // Check convergence
            let rel_residual = g[j + 1].norm() / b_norm;

            if config.print_interval > 0 && total_iterations % config.print_interval == 0 {
                log::info!(
                    "GMRES iteration {} (restart {}): relative residual = {:.6e}",
                    total_iterations,
                    restarts,
                    rel_residual.to_f64().unwrap_or(0.0)
                );
            }

            if rel_residual < config.tolerance || inner_converged {
                // Solve upper triangular system Hy = g
                let y = solve_upper_triangular(&h, &g, j + 1);

                // Update solution x = x + V * y
                for (i, &yi) in y.iter().enumerate() {
                    axpy(yi, &v[i], &mut x);
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

        for (i, &yi) in y.iter().enumerate() {
            axpy(yi, &v[i], &mut x);
        }

        restarts += 1;
    }

    // Compute final residual
    let ax = operator.apply(&x);
    let r: Array1<T> = b - &ax;
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
pub fn gmres_preconditioned<T, A, P>(
    operator: &A,
    precond: &P,
    b: &Array1<T>,
    config: &GmresConfig<T::Real>,
) -> GmresSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
    P: Preconditioner<T>,
{
    let n = b.len();
    let m = config.restart;

    let mut x = Array1::from_elem(n, T::zero());

    // Compute preconditioned RHS norm
    let pb = precond.apply(b);
    let b_norm = vector_norm(&pb);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return GmresSolution {
            x,
            iterations: 0,
            restarts: 0,
            residual: T::Real::zero(),
            converged: true,
        };
    }

    let mut total_iterations = 0;
    let mut restarts = 0;

    for _outer in 0..config.max_iterations {
        // Compute preconditioned residual r = M⁻¹(b - Ax)
        let ax = operator.apply(&x);
        let residual: Array1<T> = b - &ax;
        let r = precond.apply(&residual);
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

        let mut v: Vec<Array1<T>> = Vec::with_capacity(m + 1);
        v.push(r.mapv(|ri| ri * T::from_real(T::Real::one() / beta)));

        let mut h: Array2<T> = Array2::from_elem((m + 1, m), T::zero());
        let mut cs: Vec<T> = Vec::with_capacity(m);
        let mut sn: Vec<T> = Vec::with_capacity(m);

        let mut g: Array1<T> = Array1::from_elem(m + 1, T::zero());
        g[0] = T::from_real(beta);

        let mut inner_converged = false;

        for j in 0..m {
            total_iterations += 1;

            // w = M⁻¹ * A * v_j
            let av = operator.apply(&v[j]);
            let mut w = precond.apply(&av);

            // Modified Gram-Schmidt
            for i in 0..=j {
                h[[i, j]] = inner_product(&v[i], &w);
                let h_ij = h[[i, j]];
                w = &w - &v[i].mapv(|vi| vi * h_ij);
            }

            let w_norm = vector_norm(&w);
            h[[j + 1, j]] = T::from_real(w_norm);

            let breakdown_tol = T::Real::from_f64(1e-14).unwrap();
            if w_norm < breakdown_tol {
                inner_converged = true;
            } else {
                v.push(w.mapv(|wi| wi * T::from_real(T::Real::one() / w_norm)));
            }

            // Apply previous Givens rotations
            for i in 0..j {
                let temp = cs[i].conj() * h[[i, j]] + sn[i].conj() * h[[i + 1, j]];
                h[[i + 1, j]] = T::zero() - sn[i] * h[[i, j]] + cs[i] * h[[i + 1, j]];
                h[[i, j]] = temp;
            }

            let (c, s) = givens_rotation(h[[j, j]], h[[j + 1, j]]);
            cs.push(c);
            sn.push(s);

            h[[j, j]] = c.conj() * h[[j, j]] + s.conj() * h[[j + 1, j]];
            h[[j + 1, j]] = T::zero();

            let temp = c.conj() * g[j] + s.conj() * g[j + 1];
            g[j + 1] = T::zero() - s * g[j] + c * g[j + 1];
            g[j] = temp;

            let rel_residual = g[j + 1].norm() / b_norm;

            if rel_residual < config.tolerance || inner_converged {
                let y = solve_upper_triangular(&h, &g, j + 1);

                for (i, &yi) in y.iter().enumerate() {
                    x = &x + &v[i].mapv(|vi| vi * yi);
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
        for (i, &yi) in y.iter().enumerate() {
            x = &x + &v[i].mapv(|vi| vi * yi);
        }

        restarts += 1;
    }

    // Final residual
    let ax = operator.apply(&x);
    let residual: Array1<T> = b - &ax;
    let r = precond.apply(&residual);
    let rel_residual = vector_norm(&r) / b_norm;

    GmresSolution {
        x,
        iterations: total_iterations,
        restarts,
        residual: rel_residual,
        converged: false,
    }
}

/// GMRES solver with preconditioner and initial guess
///
/// Solves Ax = b using left preconditioning: M⁻¹Ax = M⁻¹b
/// with an optional initial guess x0.
pub fn gmres_preconditioned_with_guess<T, A, P>(
    operator: &A,
    precond: &P,
    b: &Array1<T>,
    x0: Option<&Array1<T>>,
    config: &GmresConfig<T::Real>,
) -> GmresSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
    P: Preconditioner<T>,
{
    let n = b.len();
    let m = config.restart;

    // Initialize solution vector from initial guess or zero
    let mut x = match x0 {
        Some(guess) => guess.clone(),
        None => Array1::from_elem(n, T::zero()),
    };

    // Compute preconditioned RHS norm
    let pb = precond.apply(b);
    let b_norm = vector_norm(&pb);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return GmresSolution {
            x,
            iterations: 0,
            restarts: 0,
            residual: T::Real::zero(),
            converged: true,
        };
    }

    let mut total_iterations = 0;
    let mut restarts = 0;

    for _outer in 0..config.max_iterations {
        // Compute preconditioned residual r = M⁻¹(b - Ax)
        let ax = operator.apply(&x);
        let residual: Array1<T> = b - &ax;
        let r = precond.apply(&residual);
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

        let mut v: Vec<Array1<T>> = Vec::with_capacity(m + 1);
        v.push(r.mapv(|ri| ri * T::from_real(T::Real::one() / beta)));

        let mut h: Array2<T> = Array2::from_elem((m + 1, m), T::zero());
        let mut cs: Vec<T> = Vec::with_capacity(m);
        let mut sn: Vec<T> = Vec::with_capacity(m);

        let mut g: Array1<T> = Array1::from_elem(m + 1, T::zero());
        g[0] = T::from_real(beta);

        let mut inner_converged = false;

        for j in 0..m {
            total_iterations += 1;

            // w = M⁻¹ * A * v_j
            let av = operator.apply(&v[j]);
            let mut w = precond.apply(&av);

            // Modified Gram-Schmidt
            for i in 0..=j {
                h[[i, j]] = inner_product(&v[i], &w);
                let h_ij = h[[i, j]];
                w = &w - &v[i].mapv(|vi| vi * h_ij);
            }

            let w_norm = vector_norm(&w);
            h[[j + 1, j]] = T::from_real(w_norm);

            let breakdown_tol = T::Real::from_f64(1e-14).unwrap();
            if w_norm < breakdown_tol {
                inner_converged = true;
            } else {
                v.push(w.mapv(|wi| wi * T::from_real(T::Real::one() / w_norm)));
            }

            // Apply previous Givens rotations
            for i in 0..j {
                let temp = cs[i].conj() * h[[i, j]] + sn[i].conj() * h[[i + 1, j]];
                h[[i + 1, j]] = T::zero() - sn[i] * h[[i, j]] + cs[i] * h[[i + 1, j]];
                h[[i, j]] = temp;
            }

            let (c, s) = givens_rotation(h[[j, j]], h[[j + 1, j]]);
            cs.push(c);
            sn.push(s);

            h[[j, j]] = c.conj() * h[[j, j]] + s.conj() * h[[j + 1, j]];
            h[[j + 1, j]] = T::zero();

            let temp = c.conj() * g[j] + s.conj() * g[j + 1];
            g[j + 1] = T::zero() - s * g[j] + c * g[j + 1];
            g[j] = temp;

            let rel_residual = g[j + 1].norm() / b_norm;

            if rel_residual < config.tolerance || inner_converged {
                let y = solve_upper_triangular(&h, &g, j + 1);

                for (i, &yi) in y.iter().enumerate() {
                    x = &x + &v[i].mapv(|vi| vi * yi);
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
        for (i, &yi) in y.iter().enumerate() {
            x = &x + &v[i].mapv(|vi| vi * yi);
        }

        restarts += 1;
    }

    // Final residual
    let ax = operator.apply(&x);
    let residual: Array1<T> = b - &ax;
    let r = precond.apply(&residual);
    let rel_residual = vector_norm(&r) / b_norm;

    GmresSolution {
        x,
        iterations: total_iterations,
        restarts,
        residual: rel_residual,
        converged: false,
    }
}

/// Compute Givens rotation coefficients
#[inline]
fn givens_rotation<T: ComplexField>(a: T, b: T) -> (T, T) {
    let tol = T::Real::from_f64(1e-30).unwrap();
    if b.norm() < tol {
        return (T::one(), T::zero());
    }
    if a.norm() < tol {
        return (T::zero(), T::one());
    }

    let r = (a.norm_sqr() + b.norm_sqr()).sqrt();
    let c = a * T::from_real(T::Real::one() / r);
    let s = b * T::from_real(T::Real::one() / r);

    (c, s)
}

/// Solve upper triangular system Hy = g
fn solve_upper_triangular<T: ComplexField>(h: &Array2<T>, g: &Array1<T>, k: usize) -> Vec<T> {
    let mut y = vec![T::zero(); k];
    let tol = T::Real::from_f64(1e-30).unwrap();

    for i in (0..k).rev() {
        let mut sum = g[i];
        for j in (i + 1)..k {
            sum -= h[[i, j]] * y[j];
        }
        if h[[i, i]].norm() > tol {
            y[i] = sum * h[[i, i]].inv();
        }
    }

    y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparse::CsrMatrix;
    use approx::assert_relative_eq;
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_gmres_simple() {
        let dense = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];

        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let config = GmresConfig {
            max_iterations: 100,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres(&a, &b, &config);

        assert!(solution.converged, "GMRES should converge");

        // Verify solution: Ax ≈ b
        let ax = a.matvec(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_gmres_identity() {
        let n = 5;
        let id: CsrMatrix<Complex64> = CsrMatrix::identity(n);
        let b = Array1::from_iter((1..=n).map(|i| Complex64::new(i as f64, 0.0)));

        let config = GmresConfig {
            max_iterations: 10,
            restart: 10,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let solution = gmres(&id, &b, &config);

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
    fn test_gmres_f64() {
        let dense = array![[4.0_f64, 1.0], [1.0, 3.0],];

        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![1.0_f64, 2.0];

        let config = GmresConfig {
            max_iterations: 100,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres(&a, &b, &config);

        assert!(solution.converged);

        let ax = a.matvec(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e * e).sum::<f64>().sqrt();
        assert_relative_eq!(error, 0.0, epsilon = 1e-8);
    }
}
