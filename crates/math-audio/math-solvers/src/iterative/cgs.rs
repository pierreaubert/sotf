//! CGS (Conjugate Gradient Squared) solver
//!
//! CGS is a Krylov subspace method for non-symmetric systems.
//! It can converge faster than BiCG but may be less stable.

use crate::blas_helpers::{inner_product, vector_norm};
use crate::traits::{ComplexField, LinearOperator};
use ndarray::Array1;
use num_traits::{FromPrimitive, ToPrimitive, Zero};

/// CGS solver configuration
#[derive(Debug, Clone)]
pub struct CgsConfig<R> {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Relative tolerance for convergence
    pub tolerance: R,
    /// Print progress every N iterations (0 = no output)
    pub print_interval: usize,
}

impl Default for CgsConfig<f64> {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            print_interval: 0,
        }
    }
}

/// CGS solver result
#[derive(Debug)]
pub struct CgsSolution<T: ComplexField> {
    /// Solution vector
    pub x: Array1<T>,
    /// Number of iterations
    pub iterations: usize,
    /// Final relative residual
    pub residual: T::Real,
    /// Whether convergence was achieved
    pub converged: bool,
}

/// Solve Ax = b using the CGS method
pub fn cgs<T, A>(operator: &A, b: &Array1<T>, config: &CgsConfig<T::Real>) -> CgsSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
{
    let n = b.len();
    let mut x = Array1::from_elem(n, T::zero());

    let b_norm = vector_norm(b);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        return CgsSolution {
            x,
            iterations: 0,
            residual: T::Real::zero(),
            converged: true,
        };
    }

    // Initial residual
    let mut r = b.clone();
    let r0 = r.clone(); // Shadow residual

    let mut rho = inner_product(&r0, &r);
    let mut p = r.clone();
    let mut u = r.clone();

    for iter in 0..config.max_iterations {
        // v = A * p
        let v = operator.apply(&p);

        let sigma = inner_product(&r0, &v);
        if sigma.norm() < T::Real::from_f64(1e-30).unwrap() {
            return CgsSolution {
                x,
                iterations: iter,
                residual: vector_norm(&r) / b_norm,
                converged: false,
            };
        }

        let alpha = rho / sigma;

        // q = u - alpha * v
        let q = &u - &v.mapv(|vi| vi * alpha);

        // w = A * (u + q)
        let u_plus_q = &u + &q;
        let w = operator.apply(&u_plus_q);

        // x = x + alpha * (u + q)
        x = &x + &u_plus_q.mapv(|ui| ui * alpha);

        // r = r - alpha * w
        r = &r - &w.mapv(|wi| wi * alpha);

        let rel_residual = vector_norm(&r) / b_norm;

        if config.print_interval > 0 && (iter + 1) % config.print_interval == 0 {
            log::info!(
                "CGS iteration {}: relative residual = {:.6e}",
                iter + 1,
                rel_residual.to_f64().unwrap_or(0.0)
            );
        }

        if rel_residual < config.tolerance {
            return CgsSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: true,
            };
        }

        let rho_new = inner_product(&r0, &r);
        if rho.norm() < T::Real::from_f64(1e-30).unwrap() {
            return CgsSolution {
                x,
                iterations: iter + 1,
                residual: rel_residual,
                converged: false,
            };
        }

        let beta = rho_new / rho;
        rho = rho_new;

        // u = r + beta * q
        u = &r + &q.mapv(|qi| qi * beta);

        // p = u + beta * (q + beta * p)
        let q_plus_beta_p = &q + &p.mapv(|pi| pi * beta);
        p = &u + &q_plus_beta_p.mapv(|vi| vi * beta);
    }

    let rel_residual = vector_norm(&r) / b_norm;
    CgsSolution {
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
    fn test_cgs_simple() {
        let dense = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];

        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let config = CgsConfig {
            max_iterations: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = cgs(&a, &b, &config);

        assert!(solution.converged, "CGS should converge");

        let ax = a.matvec(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }
}
