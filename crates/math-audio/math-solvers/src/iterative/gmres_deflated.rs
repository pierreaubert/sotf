//! Deflated GMRES solver for systems with clustered eigenvalues
//!
//! Deflated GMRES projects out a subspace W spanning approximations to
//! problematic eigenmodes before iterating, dramatically reducing iteration
//! counts for problems like Helmholtz where eigenvalues cluster near k².
//!
//! Reference: Erlangga & Nabben (2008), Gaul et al. (2013).

use crate::blas_helpers::{axpy, inner_product, vector_norm};
use crate::direct::LuError;
use crate::direct::lu::{LuFactorization, lu_factorize};
use crate::traits::{ComplexField, LinearOperator, Preconditioner};
use ndarray::{Array1, Array2};
use num_traits::{Float, FromPrimitive, One, ToPrimitive, Zero};

use super::gmres::{GmresConfig, GmresSolution};

/// Pre-computed deflation data for deflated GMRES
///
/// Given deflation vectors W = [w₁,...,wᵣ] (n×r, r≪n) and operator A:
/// - AW = A·W columns
/// - E = Wᴴ·A·W (r×r dense), factored via LU
///
/// Provides projectors:
/// - Left: P(v) = v - AW·E⁻¹·Wᴴ·v
/// - Coarse correction: W·E⁻¹·Wᴴ·b
/// - Recovery: Q(v) = v - W·E⁻¹·Wᴴ·A·v
#[derive(Debug, Clone)]
pub struct DeflationSubspace<T: ComplexField> {
    w_columns: Vec<Array1<T>>,
    aw_columns: Vec<Array1<T>>,
    e_lu: LuFactorization<T>,
}

impl<T: ComplexField> DeflationSubspace<T> {
    /// Construct deflation subspace from deflation vectors and operator
    ///
    /// Computes AW = A·W and factors E = Wᴴ·A·W via LU.
    ///
    /// # Arguments
    /// * `w_columns` - Deflation vectors (should be orthonormal or near-orthonormal)
    /// * `operator` - The linear operator A
    pub fn new<A: LinearOperator<T>>(
        w_columns: Vec<Array1<T>>,
        operator: &A,
    ) -> Result<Self, LuError> {
        let r = w_columns.len();
        if r == 0 {
            return Ok(Self {
                w_columns,
                aw_columns: Vec::new(),
                e_lu: LuFactorization {
                    lu: Array2::from_elem((0, 0), T::zero()),
                    pivots: Vec::new(),
                    n: 0,
                },
            });
        }

        // Compute AW columns
        let aw_columns: Vec<Array1<T>> = w_columns.iter().map(|w| operator.apply(w)).collect();

        // Build E = W^H * A * W (r x r dense)
        let mut e = Array2::from_elem((r, r), T::zero());
        for i in 0..r {
            for j in 0..r {
                e[[i, j]] = inner_product(&w_columns[i], &aw_columns[j]);
            }
        }

        let e_lu = lu_factorize(&e)?;

        Ok(Self {
            w_columns,
            aw_columns,
            e_lu,
        })
    }

    /// Number of deflation vectors
    pub fn num_vectors(&self) -> usize {
        self.w_columns.len()
    }

    /// Apply left deflation projector: P(v) = v - AW·E⁻¹·Wᴴ·v
    pub fn apply_left_projector(&self, v: &Array1<T>) -> Array1<T> {
        let r = self.w_columns.len();
        if r == 0 {
            return v.clone();
        }

        // Compute W^H * v (r-dimensional)
        let mut wh_v = Array1::from_elem(r, T::zero());
        for i in 0..r {
            wh_v[i] = inner_product(&self.w_columns[i], v);
        }

        // Solve E * y = W^H * v
        let y = self
            .e_lu
            .solve(&wh_v)
            .expect("Deflation matrix E should be non-singular");

        // result = v - AW * y
        let mut result = v.clone();
        for i in 0..r {
            axpy(-y[i], &self.aw_columns[i], &mut result);
        }

        result
    }

    /// Compute coarse correction: W·E⁻¹·Wᴴ·b
    pub fn coarse_correction(&self, b: &Array1<T>) -> Array1<T> {
        let r = self.w_columns.len();
        let n = b.len();
        if r == 0 {
            return Array1::from_elem(n, T::zero());
        }

        // Compute W^H * b
        let mut wh_b = Array1::from_elem(r, T::zero());
        for i in 0..r {
            wh_b[i] = inner_product(&self.w_columns[i], b);
        }

        // Solve E * y = W^H * b
        let y = self
            .e_lu
            .solve(&wh_b)
            .expect("Deflation matrix E should be non-singular");

        // result = W * y
        let mut result = Array1::from_elem(n, T::zero());
        for i in 0..r {
            axpy(y[i], &self.w_columns[i], &mut result);
        }

        result
    }

    /// Apply recovery operator: Q(v) = v - W·E⁻¹·Wᴴ·A·v
    pub fn apply_recovery<A: LinearOperator<T>>(&self, v: &Array1<T>, operator: &A) -> Array1<T> {
        let r = self.w_columns.len();
        if r == 0 {
            return v.clone();
        }

        // Compute A * v
        let av = operator.apply(v);

        // Compute W^H * A * v
        let mut wh_av = Array1::from_elem(r, T::zero());
        for i in 0..r {
            wh_av[i] = inner_product(&self.w_columns[i], &av);
        }

        // Solve E * y = W^H * A * v
        let y = self
            .e_lu
            .solve(&wh_av)
            .expect("Deflation matrix E should be non-singular");

        // result = v - W * y
        let mut result = v.clone();
        for i in 0..r {
            axpy(-y[i], &self.w_columns[i], &mut result);
        }

        result
    }
}

/// Solve Ax = b using deflated GMRES (no preconditioner)
///
/// Projects out the deflation subspace before each Arnoldi step.
/// After convergence, applies recovery operator and adds coarse correction.
pub fn gmres_deflated<T, A>(
    operator: &A,
    deflation: &DeflationSubspace<T>,
    b: &Array1<T>,
    x0: Option<&Array1<T>>,
    config: &GmresConfig<T::Real>,
) -> GmresSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
{
    use crate::traits::IdentityPreconditioner;
    let precond = IdentityPreconditioner;
    gmres_deflated_preconditioned(operator, &precond, deflation, b, x0, config)
}

/// Solve Ax = b using deflated preconditioned GMRES
///
/// Combines left preconditioning M⁻¹ with deflation projection P:
/// At each Arnoldi step: w = M⁻¹ · P(A · vⱼ)
/// After convergence to x̂: x = Q(x̂) + coarse_correction(b)
///
/// Reference: Erlangga & Nabben (2008)
pub fn gmres_deflated_preconditioned<T, A, P>(
    operator: &A,
    precond: &P,
    deflation: &DeflationSubspace<T>,
    b: &Array1<T>,
    x0: Option<&Array1<T>>,
    config: &GmresConfig<T::Real>,
) -> GmresSolution<T>
where
    T: ComplexField,
    A: LinearOperator<T>,
    P: Preconditioner<T>,
{
    // If no deflation vectors, fall back to standard preconditioned GMRES
    if deflation.num_vectors() == 0 {
        return super::gmres::gmres_preconditioned_with_guess(operator, precond, b, x0, config);
    }

    let n = b.len();
    let m = config.restart;

    // Compute coarse correction: x_c = W·E⁻¹·Wᴴ·b
    let x_c = deflation.coarse_correction(b);

    // Initialize solution from initial guess or zero
    let mut x_hat = match x0 {
        Some(guess) => guess.clone(),
        None => Array1::from_elem(n, T::zero()),
    };

    // Compute preconditioned deflated RHS norm for relative tolerance
    let pb = precond.apply(&deflation.apply_left_projector(b));
    let b_norm = vector_norm(&pb);
    let tol_threshold = T::Real::from_f64(1e-15).unwrap();
    if b_norm < tol_threshold {
        // Deflated RHS is essentially zero — coarse correction is the solution
        return GmresSolution {
            x: x_c,
            iterations: 0,
            restarts: 0,
            residual: T::Real::zero(),
            converged: true,
            status: crate::traits::SolverStatus::Converged,
        };
    }

    let mut total_iterations = 0;
    let mut restarts = 0;

    // Outer iteration (restarts)
    for _outer in 0..config.max_iterations {
        // Compute deflated preconditioned residual: M⁻¹ · P · (b - A · x̂)
        let ax = operator.apply(&x_hat);
        let residual: Array1<T> = b - &ax;
        let deflated_residual = deflation.apply_left_projector(&residual);
        let r = precond.apply(&deflated_residual);
        let beta = vector_norm(&r);

        let rel_residual = beta / b_norm;
        if rel_residual < config.tolerance {
            // Apply recovery and coarse correction
            let x = deflation.apply_recovery(&x_hat, operator) + &x_c;
            return GmresSolution {
                x,
                iterations: total_iterations,
                restarts,
                residual: rel_residual,
                converged: true,
                status: crate::traits::SolverStatus::Converged,
            };
        }

        // Initialize Krylov basis
        let mut v: Vec<Array1<T>> = Vec::with_capacity(m + 1);
        v.push(r.mapv(|ri| ri * T::from_real(T::Real::one() / beta)));

        // Upper Hessenberg matrix
        let mut h: Array2<T> = Array2::from_elem((m + 1, m), T::zero());

        // Givens rotation coefficients
        let mut cs: Vec<T> = Vec::with_capacity(m);
        let mut sn: Vec<T> = Vec::with_capacity(m);

        // Right-hand side of least squares problem
        let mut g: Array1<T> = Array1::from_elem(m + 1, T::zero());
        g[0] = T::from_real(beta);

        let mut inner_converged = false;

        // Inner iteration (Arnoldi process with deflation)
        for j in 0..m {
            total_iterations += 1;

            // Deflated preconditioned matvec: w = M⁻¹ · P · A · vⱼ
            let av = operator.apply(&v[j]);
            let pav = deflation.apply_left_projector(&av);
            let mut w = precond.apply(&pav);

            // Modified Gram-Schmidt orthogonalization
            for i in 0..=j {
                h[[i, j]] = inner_product(&v[i], &w);
                let h_ij = h[[i, j]];
                w = &w - &v[i].mapv(|vi| vi * h_ij);
            }

            let w_norm = vector_norm(&w);
            h[[j + 1, j]] = T::from_real(w_norm);

            // Check for breakdown
            let breakdown_tol = T::Real::from_f64(1e-20).unwrap();
            if w_norm < breakdown_tol {
                inner_converged = true;
            } else {
                v.push(w.mapv(|wi| wi * T::from_real(T::Real::one() / w_norm)));
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
            let abs_residual = g[j + 1].norm();
            let rel_residual = abs_residual / b_norm;
            let abs_tol = T::Real::from_f64(1e-20).unwrap();

            if config.print_interval > 0 && total_iterations % config.print_interval == 0 {
                log::info!(
                    "Deflated GMRES iteration {} (restart {}): relative residual = {:.6e}",
                    total_iterations,
                    restarts,
                    rel_residual.to_f64().unwrap_or(0.0)
                );
            }

            if rel_residual < config.tolerance || abs_residual < abs_tol || inner_converged {
                // Solve upper triangular system Hy = g
                let y = solve_upper_triangular(&h, &g, j + 1);

                // Update x̂ = x̂ + V * y
                for (i, &yi) in y.iter().enumerate() {
                    x_hat = &x_hat + &v[i].mapv(|vi| vi * yi);
                }

                // Apply recovery and coarse correction: x = Q(x̂) + x_c
                let x = deflation.apply_recovery(&x_hat, operator) + &x_c;

                let status = if inner_converged
                    && rel_residual >= config.tolerance
                    && abs_residual >= abs_tol
                {
                    crate::traits::SolverStatus::Breakdown
                } else {
                    crate::traits::SolverStatus::Converged
                };

                return GmresSolution {
                    x,
                    iterations: total_iterations,
                    restarts,
                    residual: rel_residual,
                    converged: true,
                    status,
                };
            }
        }

        // Maximum inner iterations reached — update x̂ and restart
        let y = solve_upper_triangular(&h, &g, m);
        for (i, &yi) in y.iter().enumerate() {
            x_hat = &x_hat + &v[i].mapv(|vi| vi * yi);
        }

        restarts += 1;
    }

    // Did not converge — still apply recovery for best-effort solution
    let x = deflation.apply_recovery(&x_hat, operator) + &x_c;

    // Compute final true residual
    let ax = operator.apply(&x);
    let r: Array1<T> = b - &ax;
    let r_norm = vector_norm(&r);
    let b_true_norm = vector_norm(b);
    let rel_residual = if b_true_norm > T::Real::from_f64(1e-15).unwrap() {
        r_norm / b_true_norm
    } else {
        r_norm
    };

    GmresSolution {
        x,
        iterations: total_iterations,
        restarts,
        residual: rel_residual,
        converged: false,
        status: crate::traits::SolverStatus::MaxIterationsReached,
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

    /// Build a diagonal operator with clustered eigenvalues near `cluster_center`
    /// plus a few well-separated eigenvalues
    fn build_clustered_diagonal(n: usize, cluster_center: f64) -> CsrMatrix<Complex64> {
        let mut dense = Array2::from_elem((n, n), Complex64::new(0.0, 0.0));
        for i in 0..n {
            if i < n - 3 {
                // Clustered eigenvalues near cluster_center
                let offset = 0.01 * (i as f64 - (n as f64 - 3.0) / 2.0);
                dense[[i, i]] = Complex64::new(cluster_center + offset, 0.0);
            } else {
                // Well-separated eigenvalues
                dense[[i, i]] = Complex64::new((i + 1) as f64 * 10.0, 0.0);
            }
        }
        CsrMatrix::from_dense(&dense, 1e-15)
    }

    #[test]
    fn test_deflation_subspace_construction() {
        // Simple 3x3 diagonal system with known eigenvectors
        let dense = array![
            [Complex64::new(1.0, 0.0), Complex64::ZERO, Complex64::ZERO],
            [Complex64::ZERO, Complex64::new(2.0, 0.0), Complex64::ZERO],
            [Complex64::ZERO, Complex64::ZERO, Complex64::new(3.0, 0.0)],
        ];
        let a = CsrMatrix::from_dense(&dense, 1e-15);

        // Deflate first eigenvector
        let w1 = array![Complex64::new(1.0, 0.0), Complex64::ZERO, Complex64::ZERO];
        let deflation = DeflationSubspace::new(vec![w1], &a).unwrap();

        assert_eq!(deflation.num_vectors(), 1);

        // P(e1) should be zero (eigenvector is fully deflated)
        let e1 = array![Complex64::new(1.0, 0.0), Complex64::ZERO, Complex64::ZERO];
        let pe1 = deflation.apply_left_projector(&e1);
        let pe1_norm: f64 = pe1.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
        assert!(pe1_norm < 1e-12, "Deflated eigenvector should be zero");

        // P(e2) should still be nonzero
        let e2 = array![Complex64::ZERO, Complex64::new(1.0, 0.0), Complex64::ZERO];
        let pe2 = deflation.apply_left_projector(&e2);
        let pe2_norm: f64 = pe2.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
        assert!(pe2_norm > 0.5, "Non-deflated vector should remain");
    }

    #[test]
    fn test_deflated_gmres_fewer_iterations() {
        let n = 20;
        let cluster = 5.0;
        let a = build_clustered_diagonal(n, cluster);

        // Build RHS
        let b = Array1::from_iter((0..n).map(|i| Complex64::new((i + 1) as f64, 0.0)));

        let config = GmresConfig {
            max_iterations: 200,
            restart: 30,
            tolerance: 1e-10,
            print_interval: 0,
        };

        // Standard GMRES
        let sol_standard = super::super::gmres::gmres(&a, &b, &config);

        // Deflated GMRES with exact eigenvectors of clustered eigenvalues
        let mut w_cols = Vec::new();
        for i in 0..(n - 3) {
            let mut w = Array1::from_elem(n, Complex64::ZERO);
            w[i] = Complex64::new(1.0, 0.0);
            w_cols.push(w);
        }
        let deflation = DeflationSubspace::new(w_cols, &a).unwrap();
        let sol_deflated = gmres_deflated(&a, &deflation, &b, None, &config);

        assert!(sol_standard.converged, "Standard GMRES should converge");
        assert!(sol_deflated.converged, "Deflated GMRES should converge");

        // Deflated version should need fewer or equal iterations
        assert!(
            sol_deflated.iterations <= sol_standard.iterations,
            "Deflated ({}) should use <= iterations than standard ({})",
            sol_deflated.iterations,
            sol_standard.iterations
        );

        // Verify solution correctness
        let ax = a.matvec(&sol_deflated.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Deflated solution should satisfy Ax = b");
    }

    #[test]
    fn test_deflated_gmres_with_preconditioner() {
        let dense = array![
            [
                Complex64::new(4.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::ZERO,
            ],
            [
                Complex64::new(1.0, 0.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(1.0, 0.0),
            ],
            [
                Complex64::ZERO,
                Complex64::new(1.0, 0.0),
                Complex64::new(5.0, 0.0),
            ],
        ];
        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0)
        ];

        // Use Jacobi preconditioner
        let precond = crate::preconditioners::DiagonalPreconditioner::from_csr(&a);

        // Single deflation vector (approximate eigenvector)
        let w1 = array![
            Complex64::new(0.5, 0.0),
            Complex64::new(0.7, 0.0),
            Complex64::new(0.5, 0.0)
        ];
        let w1_norm = vector_norm(&w1);
        let w1_normalized = w1.mapv(|v| v * Complex64::new(1.0 / w1_norm, 0.0));

        let deflation = DeflationSubspace::new(vec![w1_normalized], &a).unwrap();

        let config = GmresConfig {
            max_iterations: 50,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let sol = gmres_deflated_preconditioned(&a, &precond, &deflation, &b, None, &config);
        assert!(
            sol.converged,
            "Deflated preconditioned GMRES should converge"
        );

        // Verify solution
        let ax = a.matvec(&sol.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-8, "Solution should satisfy Ax = b");
    }

    #[test]
    fn test_deflated_gmres_zero_vectors_fallback() {
        // r=0 deflation vectors should fall back to standard GMRES
        let dense = array![
            [Complex64::new(4.0, 0.0), Complex64::new(1.0, 0.0)],
            [Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
        ];
        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let deflation = DeflationSubspace::new(Vec::new(), &a).unwrap();
        assert_eq!(deflation.num_vectors(), 0);

        let config = GmresConfig {
            max_iterations: 50,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let sol = gmres_deflated(&a, &deflation, &b, None, &config);
        assert!(sol.converged, "Zero-vector deflated GMRES should converge");

        // Should give same result as standard GMRES
        let sol_standard = super::super::gmres::gmres(&a, &b, &config);
        let error: f64 = (&sol.x - &sol_standard.x)
            .iter()
            .map(|e| e.norm_sqr())
            .sum::<f64>()
            .sqrt();
        assert!(
            error < 1e-8,
            "Zero-deflation should match standard GMRES solution"
        );
    }

    #[test]
    fn test_coarse_correction_exact_for_deflation_space() {
        // If b lies entirely in range(A·W), the coarse correction should recover it
        let dense = array![
            [Complex64::new(2.0, 0.0), Complex64::ZERO],
            [Complex64::ZERO, Complex64::new(3.0, 0.0)],
        ];
        let a = CsrMatrix::from_dense(&dense, 1e-15);

        // Deflate both eigenvectors — coarse correction solves exactly
        let w1 = array![Complex64::new(1.0, 0.0), Complex64::ZERO];
        let w2 = array![Complex64::ZERO, Complex64::new(1.0, 0.0)];
        let deflation = DeflationSubspace::new(vec![w1, w2], &a).unwrap();

        let b = array![Complex64::new(4.0, 0.0), Complex64::new(9.0, 0.0)];
        let x_c = deflation.coarse_correction(&b);

        // x_c should be the exact solution A⁻¹b = [2, 3]
        let ax_c = a.matvec(&x_c);
        let error: f64 = (&ax_c - &b)
            .iter()
            .map(|e| e.norm_sqr())
            .sum::<f64>()
            .sqrt();
        assert!(error < 1e-10, "Coarse correction should be exact solution");
    }

    #[test]
    fn test_deflated_gmres_f64() {
        let dense = array![[4.0_f64, 1.0], [1.0, 3.0],];
        let a = CsrMatrix::from_dense(&dense, 1e-15);
        let b = array![1.0_f64, 2.0];

        // Deflate one approximate eigenvector
        let w1 = array![
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2
        ];
        let w1_norm = vector_norm(&w1);
        let w1_normalized = w1.mapv(|v| v / w1_norm);
        let deflation = DeflationSubspace::new(vec![w1_normalized], &a).unwrap();

        let config = GmresConfig {
            max_iterations: 50,
            restart: 10,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let sol = gmres_deflated(&a, &deflation, &b, None, &config);
        assert!(sol.converged);

        let ax = a.matvec(&sol.x);
        let error: f64 = (&ax - &b).iter().map(|e| e * e).sum::<f64>().sqrt();
        assert_relative_eq!(error, 0.0, epsilon = 1e-8);
    }
}
