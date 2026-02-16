//! Chebyshev semi-iteration smoother for the Helmholtz operator
//!
//! Implements Chebyshev polynomial acceleration for smoothing non-characteristic
//! error modes in the Wave-ADR-NS multigrid cycle.
//!
//! The key idea: for Helmholtz A = K - k²M, eigenvalues cluster near k²
//! (characteristic) and span [0, 4/h²] (non-characteristic). Chebyshev
//! acceleration targets the non-characteristic modes using polynomial bounds.
//!
//! Reference: Cui & Jiang (2024, arXiv:2404.02493), Section 3.1

use math_audio_solvers::CsrMatrix;
use ndarray::Array1;
use num_complex::Complex64;

/// Apply Chebyshev semi-iteration smoothing to the normal equations A^H A x = A^H b
///
/// Uses the three-term Chebyshev recurrence to accelerate convergence on
/// non-characteristic error components. The recurrence is:
///   x^{q+1} = x^q + beta_q * (A^H * b - A^H * A * x^q)
/// where beta_q are Chebyshev coefficients based on eigenvalue bounds.
///
/// # Arguments
/// * `a` - System matrix (Helmholtz operator)
/// * `x` - Current solution (modified in place)
/// * `b` - Right-hand side
/// * `iterations` - Number of Chebyshev steps
/// * `alpha` - Chebyshev parameter alpha controlling the excluded eigenvalue region
pub fn chebyshev_smooth(
    a: &CsrMatrix<Complex64>,
    x: &mut Array1<Complex64>,
    b: &Array1<Complex64>,
    iterations: usize,
    alpha: f64,
) {
    if iterations == 0 {
        return;
    }

    let n = x.len();

    // Estimate spectral bounds for A^H A
    // lambda_max(A^H A) ~ ||A||_F^2 / n (rough Frobenius estimate)
    // We use a more conservative estimate based on the diagonal
    let lambda_max = estimate_spectral_radius(a);

    // Chebyshev parameters for interval [alpha * lambda_max, lambda_max]
    // d = (lambda_max + alpha * lambda_max) / 2 = lambda_max * (1 + alpha) / 2
    // c = (lambda_max - alpha * lambda_max) / 2 = lambda_max * (1 - alpha) / 2
    let d = lambda_max * (1.0 + alpha) / 2.0;
    let c = lambda_max * (1.0 - alpha) / 2.0;

    if d.abs() < 1e-15 || c.abs() < 1e-15 {
        return;
    }

    // Compute A^H b once
    let ah_b = matvec_hermitian(a, b);

    // First iteration: x = x + (1/d) * (A^H b - A^H A x)
    let ah_ax = matvec_hermitian(a, &a.matvec(x));
    let mut residual = &ah_b - &ah_ax;
    let beta_0 = 1.0 / d;
    *x = &*x + &(residual.mapv(|v| v * beta_0));

    if iterations == 1 {
        return;
    }

    // Subsequent iterations use three-term recurrence
    let mut x_prev = x.clone();
    let mut rho_prev = 1.0_f64;

    for _q in 1..iterations {
        // rho_q = 1 / (2*d/c - rho_{q-1})
        let rho = 1.0 / (2.0 * d / c - rho_prev);

        // Compute residual of normal equations: A^H b - A^H A x
        let ah_ax = matvec_hermitian(a, &a.matvec(x));
        residual = &ah_b - &ah_ax;

        // Numerically stable three-term recurrence formulation:
        // x_{q+1} = (1 + rho*(rho_prev - 1)) * x_q - rho*(rho_prev - 1) * x_{q-1}
        //           + 2*rho/c * (A^H b - A^H A x_q)
        let factor1 = 1.0 + rho * (rho_prev - 1.0);
        let factor2 = rho * (rho_prev - 1.0);
        let factor3 = 2.0 * rho / c;

        let x_next = Array1::from_shape_fn(n, |i| {
            Complex64::new(factor1, 0.0) * x[i] - Complex64::new(factor2, 0.0) * x_prev[i]
                + Complex64::new(factor3, 0.0) * residual[i]
        });

        x_prev = x.clone();
        *x = x_next;
        rho_prev = rho;
    }
}

/// Apply damped Jacobi smoothing optimized for the finest grid
///
/// Uses the Helmholtz-optimal damping factor:
///   omega_0 = (2 - k²h²) / (3 - k²h²)
/// which accounts for the negative mass term in the Helmholtz operator.
///
/// # Arguments
/// * `a` - System matrix in CSR format
/// * `x` - Current solution (modified in place)
/// * `b` - Right-hand side
/// * `omega` - Damping factor (use `optimal_jacobi_damping` for Helmholtz-optimal value)
pub fn jacobi_damped_smooth(
    a: &CsrMatrix<Complex64>,
    x: &mut Array1<Complex64>,
    b: &Array1<Complex64>,
    omega: f64,
) {
    let n = x.len();

    // Extract diagonal
    let diag = extract_diagonal(a);

    // Compute r = b - A*x
    let ax = a.matvec(x);
    let residual: Array1<Complex64> = b - &ax;

    // Update: x = x + omega * D^{-1} * r
    for i in 0..n {
        if diag[i].norm() > 1e-15 {
            x[i] += Complex64::new(omega, 0.0) * residual[i] / diag[i];
        }
    }
}

/// Compute the optimal Jacobi damping factor for Helmholtz on level with mesh size h
///
/// omega_0 = (2 - k²h²) / (3 - k²h²)
///
/// For kh < sqrt(2), this gives a positive damping factor.
/// For kh >= sqrt(2), falls back to standard 2/3 damping.
pub fn optimal_jacobi_damping(k: f64, h: f64) -> f64 {
    let kh_sq = k * k * h * h;
    if kh_sq < 2.0 {
        (2.0 - kh_sq) / (3.0 - kh_sq)
    } else {
        2.0 / 3.0
    }
}

/// Estimate the Chebyshev parameter alpha for a given multigrid level
///
/// Alpha determines the fraction of the spectrum excluded by the Chebyshev polynomial.
/// For Helmholtz, the characteristic eigenvalues cluster near k², so:
///   alpha ~ (k*h_level)^2 / 4
///
/// This ensures the Chebyshev polynomial damps eigenvalues away from the
/// characteristic region while leaving the characteristic modes for ADR correction.
pub fn estimate_alpha(k: f64, h: f64, _level: usize) -> f64 {
    let kh = k * h;
    // Alpha bounds the lower end of the Chebyshev target interval
    // For Helmholtz: eigenvalues of A^H A in [alpha * lambda_max, lambda_max]
    // We exclude the characteristic region near k^2 by setting alpha ~ (kh)^2 / 4
    let alpha = (kh * kh / 4.0).min(0.9);
    alpha.max(0.01) // Keep alpha bounded away from 0 and 1
}

/// Compute A^H * x (Hermitian transpose matrix-vector product)
fn matvec_hermitian(a: &CsrMatrix<Complex64>, x: &Array1<Complex64>) -> Array1<Complex64> {
    let n = a.num_cols;
    let mut y = Array1::zeros(n);

    // A^H = conjugate transpose: y[col] += conj(A[row,col]) * x[row]
    for row in 0..a.num_rows {
        let start = a.row_ptrs[row];
        let end = a.row_ptrs[row + 1];
        for idx in start..end {
            let col = a.col_indices[idx];
            let val = a.values[idx].conj();
            y[col] += val * x[row];
        }
    }

    y
}

/// Extract diagonal of CSR matrix
fn extract_diagonal(a: &CsrMatrix<Complex64>) -> Vec<Complex64> {
    let n = a.num_rows;
    let mut diag = vec![Complex64::new(0.0, 0.0); n];

    for (row, diag_entry) in diag.iter_mut().enumerate() {
        let start = a.row_ptrs[row];
        let end = a.row_ptrs[row + 1];
        for idx in start..end {
            if a.col_indices[idx] == row {
                *diag_entry = a.values[idx];
            }
        }
    }

    diag
}

/// Estimate spectral radius of A^H A using Gershgorin circle theorem on diagonal
fn estimate_spectral_radius(a: &CsrMatrix<Complex64>) -> f64 {
    let mut max_radius = 0.0_f64;

    for row in 0..a.num_rows {
        let start = a.row_ptrs[row];
        let end = a.row_ptrs[row + 1];
        let mut row_sum = 0.0;
        for idx in start..end {
            row_sum += a.values[idx].norm_sqr();
        }
        max_radius = max_radius.max(row_sum);
    }

    max_radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_optimal_jacobi_damping() {
        // Small kh -> close to 2/3
        let omega = optimal_jacobi_damping(1.0, 0.1);
        assert!(omega > 0.5 && omega < 1.0);

        // Large kh -> fallback to 2/3
        let omega_large = optimal_jacobi_damping(10.0, 1.0);
        assert!((omega_large - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_estimate_alpha() {
        let alpha = estimate_alpha(1.0, 0.1, 0);
        assert!(alpha >= 0.01);
        assert!(alpha <= 0.9);

        // Higher kh -> larger alpha
        let alpha_high = estimate_alpha(10.0, 0.5, 0);
        assert!(alpha_high > alpha);
    }

    #[test]
    fn test_chebyshev_reduces_residual() {
        let mesh = unit_square_triangles(8);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |x, y, _z| {
            Complex64::new(
                (x * std::f64::consts::PI).sin() * (y * std::f64::consts::PI).sin(),
                0.0,
            )
        });

        let csr = problem.matrix.to_csr();
        let rhs = Array1::from(problem.rhs.clone());
        let mut x = Array1::zeros(rhs.len());

        let initial_residual = compute_residual_norm(&csr, &x, &rhs);

        chebyshev_smooth(&csr, &mut x, &rhs, 5, 0.1);

        let final_residual = compute_residual_norm(&csr, &x, &rhs);
        assert!(
            final_residual < initial_residual,
            "Chebyshev should reduce residual: {} -> {}",
            initial_residual,
            final_residual
        );
    }

    #[test]
    fn test_jacobi_smooth_reduces_residual() {
        // Use a positive-definite system (k=0 → pure Laplacian) to test Jacobi convergence
        // Jacobi is not guaranteed to reduce residual on indefinite Helmholtz systems
        let mesh = unit_square_triangles(8);
        let k = Complex64::new(0.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let csr = problem.matrix.to_csr();
        let rhs = Array1::from(problem.rhs.clone());
        let mut x = Array1::zeros(rhs.len());

        let initial_residual = compute_residual_norm(&csr, &x, &rhs);

        for _ in 0..10 {
            jacobi_damped_smooth(&csr, &mut x, &rhs, 2.0 / 3.0);
        }

        let final_residual = compute_residual_norm(&csr, &x, &rhs);
        assert!(
            final_residual < initial_residual,
            "Jacobi should reduce residual on Laplacian: {} -> {}",
            initial_residual,
            final_residual
        );
    }

    fn compute_residual_norm(
        a: &CsrMatrix<Complex64>,
        x: &Array1<Complex64>,
        b: &Array1<Complex64>,
    ) -> f64 {
        let ax = a.matvec(x);
        let residual = b - &ax;
        residual.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt()
    }
}
