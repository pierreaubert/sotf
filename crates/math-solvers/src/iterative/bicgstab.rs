//! BiCGSTAB (Bi-Conjugate Gradient Stabilized) solver
//!
//! BiCGSTAB is a Krylov subspace method for non-symmetric systems.
//! It often converges faster than GMRES for certain problem types.

use crate::blas_helpers::{inner_product, vector_norm};
use crate::traits::{ComplexField, LinearOperator};
use ndarray::Array1;
use num_traits::{FromPrimitive, ToPrimitive, Zero};

/// BiCGSTAB solver configuration
#[derive(Debug, Clone)]
pub struct BiCgstabConfig<R> {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Relative tolerance for convergence
    pub tolerance: R,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for BiCgstabConfig<f64> {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            print_interval: 0,
        }
    }
}

/// BiCGSTAB solver result
#[derive(Debug)]
pub struct BiCgstabSolution<T: ComplexField> {
    /// Solution vector
    pub x: Array1<T>,
    /// Number of iterations
    pub iterations: usize,
    /// Final relative residual
    pub residual: T::Real,
    /// Whether convergence was achieved
    pub converged: bool,
}

/// Solve Ax = b using the BiCGSTAB method
pub fn bicgstab<T, A>(
    operator: &A,
    b: &Array1<T>,
    config: &BiCgstabConfig<T::Real>,
) -> BiCgstabSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
{
    let n = b.len();
    let mut x = Array1::from_elem(n, T::zero());

    let b_norm = vector_norm(b);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return BiCgstabSolution {
            x,
            iterations: 0,
            residual: T::Real::zero(),
            converged: true,
        };
    }

    // Initial residual
    let mut r = b.clone();
    let r0 = r.clone(); // Shadow residual

    let mut rho = T::one();
    let mut alpha = T::one();
    let mut omega = T::one();

    let mut p = Array1::from_elem(n, T::zero());
    let mut v = Array1::from_elem(n, T::zero());

    for iter in 0..config.max_iterations {
        let rho_new = inner_product(&r0, &r);

        // Check for breakdown
        if rho_new.norm() < T::Real::from_f64(1e-30).unwrap() {
            return BiCgstabSolution {
                x,
                iterations: iter,
                residual: vector_norm(&r) / b_norm,
                converged: false,
            };
        }

        let beta = (rho_new / rho) * (alpha / omega);
        rho = rho_new;

        // p = r + beta * (p - omega * v)
        p = &r + &(&p - &v.mapv(|vi| vi * omega)).mapv(|pi| pi * beta);

        // v = A * p
        v = operator.apply(&p);

        let r0v = inner_product(&r0, &v);
        if r0v.norm() < T::Real::from_f64(1e-30).unwrap() {
            return BiCgstabSolution {
                x,
                iterations: iter,
                residual: vector_norm(&r) / b_norm,
                converged: false,
            };
        }

        alpha = rho / r0v;

        // s = r - alpha * v
        let s = &r - &v.mapv(|vi| vi * alpha);

        // Check for early convergence
        let s_norm = vector_norm(&s);
        if s_norm / b_norm < config.tolerance {
            x = &x + &p.mapv(|pi| pi * alpha);
            return BiCgstabSolution {
                x,
                iterations: iter + 1,
                residual: s_norm / b_norm,
                converged: true,
            };
        }

        // t = A * s
        let t = operator.apply(&s);

        // omega = (t, s) / (t, t)
        let tt = inner_product(&t, &t);
        if tt.norm() < T::Real::from_f64(1e-30).unwrap() {
            return BiCgstabSolution {
                x,
                iterations: iter,
                residual: vector_norm(&r) / b_norm,
                converged: false,
            };
        }
        omega = inner_product(&t, &s) / tt;

        // x = x + alpha * p + omega * s
        x = &x + &p.mapv(|pi| pi * alpha) + &s.mapv(|si| si * omega);

        // r = s - omega * t
        r = &s - &t.mapv(|ti| ti * omega);

        let rel_residual = vector_norm(&r) / b_norm;

        if config.print_interval > 0 && (iter + 1) % config.print_interval == 0 {
            log::info!(
                "BiCGSTAB iteration {}: relative residual = {:.6e}",
                iter + 1,
                rel_residual.to_f64().unwrap_or(0.0)
            );
        }

        if rel_residual < config.tolerance {
            return BiCgstabSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: true,
            };
        }

        // Check for stagnation
        if omega.norm() < T::Real::from_f64(1e-30).unwrap() {
            return BiCgstabSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: false,
            };
        }
    }

    let rel_residual = vector_norm(&r) / b_norm;
    BiCgstabSolution {
        x,
        iterations: config.max_iterations,
        residual: rel_residual,
        converged: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparse::CsrMatrix;
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_bicgstab_simple() {
        let dense = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];

        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let config = BiCgstabConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = bicgstab(&a, &b, &config);

        assert!(solution.converged, "BiCGSTAB should converge");

        let ax = a.matvec(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }
}
