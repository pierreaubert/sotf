//! Pipelined GMRES solver (p-GMRES)
//!
//! Implementation of the Pipelined GMRES algorithm based on Ghysels et al. (2013).
//! This solver overlaps the matrix-vector product with global reductions to hide communication latency
//! and improve scalability on parallel systems.

use crate::iterative::gmres::{GmresConfig, GmresSolution};
use crate::traits::{ComplexField, LinearOperator, Preconditioner};
use ndarray::{Array1, Array2};
use num_traits::{Float, FromPrimitive, One, ToPrimitive, Zero};
use rayon::prelude::*;

/// Solve Ax = b using the Pipelined GMRES method
///
/// This moves the SpMV of the *next* iteration to overlap with the dot products of the *current* iteration.
/// It requires maintaining an auxiliary basis Z = AV.
pub fn gmres_pipelined<T, A, P>(
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

    // Initialize solution vector
    let mut x = match x0 {
        Some(x0) => x0.clone(),
        None => Array1::from_elem(n, T::zero()),
    };

    // Compute initial residual
    // r0 = b - Ax0
    let ax = operator.apply(&x);
    let r0: Array1<T> = b - &ax;

    // Apply preconditioner (Left preconditioning: M^-1 A x = M^-1 b)
    let r0 = precond.apply(&r0);

    let b_norm = vector_norm(&r0);
    // Use proper tolerance check from config or default
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

    // Vectors
    let mut v: Vec<Array1<T>> = Vec::with_capacity(m + 1);
    let mut z: Vec<Array1<T>> = Vec::with_capacity(m + 1);

    for _outer in 0..config.max_iterations {
        // Compute residual (or reuse)
        // Correct restart logic: r = M^-1 (b - Ax)
        let ax = operator.apply(&x);
        let residual = b - &ax;
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

        v.clear();
        z.clear();

        // v[0] = r / beta
        v.push(r.mapv(|ri| ri * T::from_real(T::Real::one() / beta)));

        // Pre-calculate z[0] = M^-1 A v[0]
        let av = operator.apply(&v[0]);
        z.push(precond.apply(&av));

        let mut h: Array2<T> = Array2::from_elem((m + 1, m), T::zero());
        let mut g: Array1<T> = Array1::from_elem(m + 1, T::zero());
        g[0] = T::from_real(beta);

        let mut cs: Vec<T> = Vec::with_capacity(m);
        let mut sn: Vec<T> = Vec::with_capacity(m);

        let mut inner_converged = false;

        for j in 0..m {
            total_iterations += 1;

            // Pipelined step: Overlap SpMV and Reductions
            let (q, h_col) = rayon::join(
                || {
                    let az = operator.apply(&z[j]);
                    precond.apply(&az)
                },
                || {
                    // Compute column of H: dot products of z[j] with all v[0]..v[j]
                    (0..=j)
                        .into_par_iter()
                        .map(|i| inner_product(&v[i], &z[j]))
                        .collect::<Vec<_>>()
                },
            );

            // Place results into H
            for i in 0..=j {
                h[[i, j]] = h_col[i];
            }

            // Update v[j+1] and z[j+1]
            // v_new = z[j] - sum(h_ij * v_i)
            // z_new = q - sum(h_ij * z_i)

            let mut v_new = z[j].clone();
            let mut z_new = q;

            for i in 0..=j {
                let factor = h[[i, j]];
                v_new = &v_new - &v[i].mapv(|val| val * factor);
                z_new = &z_new - &z[i].mapv(|val| val * factor);
            }

            // Compute norm h[j+1, j] = ||v_new||
            let norm_v = vector_norm(&v_new);
            h[[j + 1, j]] = T::from_real(norm_v);

            // Breakdown check
            let breakdown_tol = T::Real::from_f64(1e-14).unwrap();

            if norm_v < breakdown_tol {
                inner_converged = true;
            } else {
                // Normalize
                let inv_norm = T::from_real(T::Real::one() / norm_v);
                v.push(v_new.mapv(|val| val * inv_norm));
                z.push(z_new.mapv(|val| val * inv_norm));
            }

            // Givens rotations (same as standard GMRES)
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

            if config.print_interval > 0 && total_iterations % config.print_interval == 0 {
                log::info!(
                    "p-GMRES iteration {} (restart {}): relative residual = {:.6e}",
                    total_iterations,
                    restarts,
                    rel_residual.to_f64().unwrap_or(0.0)
                );
            }

            if rel_residual < config.tolerance || inner_converged {
                let y = solve_upper_triangular(&h, &g, j + 1);
                for (i, &yi) in y.iter().enumerate() {
                    // x = x + v[i] * yi
                    x = &x + &v[i].mapv(|val| val * yi);
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
            x = &x + &v[i].mapv(|val| val * yi);
        }
        restarts += 1;
    }

    // Final check
    let ax = operator.apply(&x);
    let r = b - &ax;
    let r = precond.apply(&r);
    let rel_residual = vector_norm(&r) / b_norm;

    GmresSolution {
        x,
        iterations: total_iterations,
        restarts,
        residual: rel_residual,
        converged: false,
    }
}

// Helpers

#[inline]
fn inner_product<T: ComplexField>(x: &Array1<T>, y: &Array1<T>) -> T {
    x.iter()
        .zip(y.iter())
        .fold(T::zero(), |acc, (&xi, &yi)| acc + xi.conj() * yi)
}

#[inline]
fn vector_norm<T: ComplexField>(x: &Array1<T>) -> T::Real {
    x.iter()
        .map(|xi| xi.norm_sqr())
        .fold(T::Real::zero(), |acc, val| acc + val)
        .sqrt()
}

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
    use crate::preconditioners::IdentityPreconditioner;
    use crate::sparse::CsrMatrix;
    use ndarray::array;
    use num_complex::Complex64;

    #[test]
    fn test_pgmres_simple_solver() {
        let dense = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];

        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];
        let precond = IdentityPreconditioner;

        let config = GmresConfig {
            max_iterations: 100,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres_pipelined(&a, &precond, &b, None, &config);

        assert!(solution.converged, "GMRES should converge");

        // Verify solution: Ax ≈ b
        let ax = a.apply(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }
}
