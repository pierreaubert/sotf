//! Direct solver (LU factorization)
//!
//! Uses ndarray-linalg for LU decomposition and solve when `native` feature is enabled,
//! falls back to pure Rust implementation otherwise.
//!
//! The pure Rust fallback is slower but allows BEM code to work in WASM and other
//! environments without native BLAS.

use ndarray::{Array1, Array2};
use num_complex::Complex64;

/// Direct solver result
#[derive(Debug)]
pub struct DirectSolution {
    /// Solution vector
    pub x: Array1<Complex64>,
    /// Whether the solve was successful
    pub success: bool,
}

/// Solve Ax = b using LU factorization
///
/// Uses BLAS when `native` feature is enabled, pure Rust otherwise.
///
/// # Arguments
/// * `a` - Coefficient matrix (n × n)
/// * `b` - Right-hand side vector (n)
///
/// # Returns
/// Solution struct containing x and success status
///
/// # Example
/// ```ignore
/// let solution = direct_solve(&matrix, &rhs);
/// if solution.success {
///     println!("Solution: {:?}", solution.x);
/// }
/// ```
#[cfg(feature = "native")]
pub fn direct_solve(a: &Array2<Complex64>, b: &Array1<Complex64>) -> DirectSolution {
    use ndarray_linalg::Solve;

    match a.solve(b) {
        Ok(x) => DirectSolution { x, success: true },
        Err(_) => DirectSolution {
            x: Array1::zeros(b.len()),
            success: false,
        },
    }
}

/// Solve Ax = b using LU factorization (pure Rust fallback)
///
/// This version is used when the `native` feature is disabled.
/// Slower than BLAS but works in WASM and other restricted environments.
#[cfg(not(feature = "native"))]
pub fn direct_solve(a: &Array2<Complex64>, b: &Array1<Complex64>) -> DirectSolution {
    use crate::core::algebra;

    match algebra::solve(a, b) {
        Some(x) => DirectSolution { x, success: true },
        None => DirectSolution {
            x: Array1::zeros(b.len()),
            success: false,
        },
    }
}

/// Solve Ax = b using LU factorization with pivoting
///
/// This is an alternative interface that's equivalent to direct_solve.
pub fn direct_solve_lu(a: &Array2<Complex64>, b: &Array1<Complex64>) -> DirectSolution {
    // Just delegate to the main solve function
    direct_solve(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_solve_simple() {
        // Simple 2x2 system
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(2.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(3.0, 0.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![Complex64::new(4.0, 0.0), Complex64::new(5.0, 0.0)]);

        let solution = direct_solve(&a, &b);

        assert!(solution.success);

        // Verify: Ax ≈ b
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10);
    }

    #[test]
    fn test_direct_solve_complex() {
        // Complex system
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(1.0, 1.0),
                Complex64::new(0.0, 1.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, -1.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![Complex64::new(2.0, 1.0), Complex64::new(1.0, 0.0)]);

        let solution = direct_solve(&a, &b);

        assert!(solution.success);

        // Verify: Ax ≈ b
        let ax = a.dot(&solution.x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10);
    }

    #[test]
    fn test_direct_solve_identity() {
        let n = 5;
        let mut a: Array2<Complex64> = Array2::zeros((n, n));
        for i in 0..n {
            a[[i, i]] = Complex64::new(1.0, 0.0);
        }

        let b = Array1::from_vec(
            (1..=n)
                .map(|i| Complex64::new(i as f64, 0.0))
                .collect::<Vec<_>>(),
        );

        let solution = direct_solve(&a, &b);

        assert!(solution.success);

        // x should equal b for identity matrix
        let error: f64 = (&solution.x - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10);
    }
}
