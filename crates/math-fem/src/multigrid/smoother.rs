//! Smoothers for multigrid methods
//!
//! Provides Gauss-Seidel and Jacobi smoothers for reducing high-frequency error.

use crate::assembly::HelmholtzMatrix;
use num_complex::Complex64;
use std::collections::HashMap;

/// Smoother type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmootherType {
    /// Gauss-Seidel (sequential)
    GaussSeidel,
    /// Damped Jacobi
    Jacobi,
    /// Symmetric Gauss-Seidel (forward then backward)
    SymmetricGaussSeidel,
}

/// Smoother configuration
#[derive(Debug, Clone)]
pub struct SmootherConfig {
    /// Type of smoother
    pub smoother_type: SmootherType,
    /// Number of smoothing iterations
    pub iterations: usize,
    /// Damping factor (for Jacobi)
    pub omega: f64,
}

impl Default for SmootherConfig {
    fn default() -> Self {
        Self {
            smoother_type: SmootherType::GaussSeidel,
            iterations: 2,
            omega: 2.0 / 3.0, // Optimal for Laplacian
        }
    }
}

/// Apply smoothing iterations to reduce high-frequency error
///
/// Solves A*x = b approximately, updating x in place
pub fn smooth(
    matrix: &HelmholtzMatrix,
    x: &mut [Complex64],
    b: &[Complex64],
    config: &SmootherConfig,
) {
    match config.smoother_type {
        SmootherType::GaussSeidel => {
            for _ in 0..config.iterations {
                gauss_seidel_sweep(matrix, x, b, false);
            }
        }
        SmootherType::Jacobi => {
            for _ in 0..config.iterations {
                jacobi_sweep(matrix, x, b, config.omega);
            }
        }
        SmootherType::SymmetricGaussSeidel => {
            for _ in 0..config.iterations {
                gauss_seidel_sweep(matrix, x, b, false);
                gauss_seidel_sweep(matrix, x, b, true);
            }
        }
    }
}

/// Single Gauss-Seidel sweep
fn gauss_seidel_sweep(
    matrix: &HelmholtzMatrix,
    x: &mut [Complex64],
    b: &[Complex64],
    backward: bool,
) {
    #[allow(unused_variables)]
    let n = matrix.dim;

    // Build CSR-like structure for efficient row access
    let mut row_data: HashMap<usize, Vec<(usize, Complex64)>> = HashMap::new();
    let mut diagonals: Vec<Complex64> = vec![Complex64::new(0.0, 0.0); n];

    for k in 0..matrix.rows.len() {
        let row = matrix.rows[k];
        let col = matrix.cols[k];
        let val = matrix.values[k];

        if row == col {
            diagonals[row] += val;
        } else {
            row_data.entry(row).or_default().push((col, val));
        }
    }

    // Sweep order
    let indices: Box<dyn Iterator<Item = usize>> = if backward {
        Box::new((0..n).rev())
    } else {
        Box::new(0..n)
    };

    for i in indices {
        if diagonals[i].norm() < 1e-15 {
            continue; // Skip zero diagonal
        }

        let mut sigma = Complex64::new(0.0, 0.0);
        if let Some(row_entries) = row_data.get(&i) {
            for &(j, val) in row_entries {
                sigma += val * x[j];
            }
        }

        x[i] = (b[i] - sigma) / diagonals[i];
    }
}

/// Single Jacobi sweep with damping
fn jacobi_sweep(matrix: &HelmholtzMatrix, x: &mut [Complex64], b: &[Complex64], omega: f64) {
    #[allow(unused_variables)]
    let n = matrix.dim;
    // Build row structure
    let mut row_data: HashMap<usize, Vec<(usize, Complex64)>> = HashMap::new();
    let mut diagonals: Vec<Complex64> = vec![Complex64::new(0.0, 0.0); n];

    for k in 0..matrix.rows.len() {
        let row = matrix.rows[k];
        let col = matrix.cols[k];
        let val = matrix.values[k];

        if row == col {
            diagonals[row] += val;
        } else {
            row_data.entry(row).or_default().push((col, val));
        }
    }

    // Compute new values (all based on old x)
    let mut x_new = vec![Complex64::new(0.0, 0.0); n];

    for i in 0..n {
        if diagonals[i].norm() < 1e-15 {
            x_new[i] = x[i];
            continue;
        }

        let mut sigma = Complex64::new(0.0, 0.0);
        if let Some(row_entries) = row_data.get(&i) {
            for &(j, val) in row_entries {
                sigma += val * x[j];
            }
        }

        let x_gs = (b[i] - sigma) / diagonals[i];
        x_new[i] = Complex64::new(omega, 0.0) * x_gs + Complex64::new(1.0 - omega, 0.0) * x[i];
    }

    x.copy_from_slice(&x_new);
}

/// Compute residual r = b - A*x
pub fn compute_residual(
    matrix: &HelmholtzMatrix,
    x: &[Complex64],
    b: &[Complex64],
) -> Vec<Complex64> {
    let mut r = b.to_vec();

    // r = b - A*x
    for k in 0..matrix.rows.len() {
        r[matrix.rows[k]] -= matrix.values[k] * x[matrix.cols[k]];
    }

    r
}

/// Compute residual norm
pub fn residual_norm(matrix: &HelmholtzMatrix, x: &[Complex64], b: &[Complex64]) -> f64 {
    let r = compute_residual(matrix, x, b);
    r.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_gauss_seidel_reduces_residual() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(0.0, 0.0); // Laplacian

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |x, y, _z| {
            Complex64::new((x * y).sin(), 0.0)
        });

        let matrix = problem.matrix.to_compressed();
        let b = &problem.rhs;

        let mut x = vec![Complex64::new(0.0, 0.0); matrix.dim];
        let initial_norm = residual_norm(&matrix, &x, b);

        let config = SmootherConfig::default();
        smooth(&matrix, &mut x, b, &config);

        let final_norm = residual_norm(&matrix, &x, b);
        assert!(final_norm < initial_norm, "Residual should decrease");
    }

    #[test]
    fn test_jacobi_with_damping() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(0.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let matrix = problem.matrix.to_compressed();
        let b = &problem.rhs;

        let mut x = vec![Complex64::new(0.0, 0.0); matrix.dim];
        let initial_norm = residual_norm(&matrix, &x, b);

        let config = SmootherConfig {
            smoother_type: SmootherType::Jacobi,
            iterations: 5,
            omega: 0.6,
        };
        smooth(&matrix, &mut x, b, &config);

        let final_norm = residual_norm(&matrix, &x, b);
        assert!(final_norm < initial_norm, "Jacobi should reduce residual");
    }
}
