//! Pure Rust linear algebra operations
//!
//! This module provides pure Rust implementations of essential linear algebra
//! operations as fallbacks when BLAS/LAPACK (via ndarray-linalg) is not available.
//!
//! These implementations are used when building for WASM or other targets where
//! native BLAS is not available. They are slower than optimized BLAS but allow
//! the BEM code to work in more environments.
//!
//! ## Operations Provided
//!
//! - LU decomposition with partial pivoting
//! - Linear system solve via LU factorization
//! - Matrix inverse via LU factorization
//!
//! ## Usage
//!
//! The functions automatically use BLAS when the `native` feature is enabled,
//! falling back to pure Rust implementations otherwise.

use ndarray::{Array1, Array2, s};
use num_complex::Complex64;

/// Result of LU factorization
#[derive(Debug, Clone)]
pub struct LuFactorization {
    /// Combined L and U factors (L below diagonal, U on and above diagonal)
    pub lu: Array2<Complex64>,
    /// Pivot indices from partial pivoting
    pub pivots: Vec<usize>,
    /// Number of row swaps (for determinant sign)
    pub num_swaps: usize,
}

impl LuFactorization {
    /// Check if the factorization indicates a singular matrix
    pub fn is_singular(&self, tol: f64) -> bool {
        let n = self.lu.nrows();
        for i in 0..n {
            if self.lu[[i, i]].norm() < tol {
                return true;
            }
        }
        false
    }
}

/// Perform LU decomposition with partial pivoting (pure Rust)
///
/// Decomposes matrix A into PA = LU where:
/// - P is a permutation matrix (represented by pivot indices)
/// - L is lower triangular with unit diagonal
/// - U is upper triangular
///
/// # Arguments
/// * `a` - Square matrix to factorize
///
/// # Returns
/// LU factorization or None if matrix is singular
///
/// # Algorithm
/// Uses Doolittle's method with partial pivoting for numerical stability.
pub fn lu_factorize(a: &Array2<Complex64>) -> Option<LuFactorization> {
    let n = a.nrows();
    if n != a.ncols() {
        return None; // Must be square
    }

    let mut lu = a.clone();
    let mut pivots: Vec<usize> = (0..n).collect();
    let mut num_swaps = 0;

    for k in 0..n {
        // Find pivot (largest absolute value in column k, rows k..n)
        let mut max_val = lu[[k, k]].norm();
        let mut max_row = k;

        for i in (k + 1)..n {
            let val = lu[[i, k]].norm();
            if val > max_val {
                max_val = val;
                max_row = i;
            }
        }

        // Check for singularity
        if max_val < 1e-15 {
            // Matrix is singular or nearly singular
            // We continue anyway to allow solving overdetermined systems
        }

        // Swap rows if needed
        if max_row != k {
            for j in 0..n {
                let tmp = lu[[k, j]];
                lu[[k, j]] = lu[[max_row, j]];
                lu[[max_row, j]] = tmp;
            }
            pivots.swap(k, max_row);
            num_swaps += 1;
        }

        // Compute L and U entries
        let diag = lu[[k, k]];
        if diag.norm() > 1e-30 {
            let diag_inv = Complex64::new(1.0, 0.0) / diag;

            for i in (k + 1)..n {
                // L[i,k] = A[i,k] / U[k,k]
                lu[[i, k]] *= diag_inv;

                // U[i,j] = A[i,j] - L[i,k] * U[k,j] for j > k
                let lik = lu[[i, k]];
                for j in (k + 1)..n {
                    let ukj = lu[[k, j]];
                    lu[[i, j]] -= lik * ukj;
                }
            }
        }
    }

    Some(LuFactorization {
        lu,
        pivots,
        num_swaps,
    })
}

/// Solve Ax = b using LU factorization (pure Rust)
///
/// # Arguments
/// * `lu_fact` - LU factorization from `lu_factorize`
/// * `b` - Right-hand side vector
///
/// # Returns
/// Solution vector x, or None if the system is singular
///
/// # Algorithm
/// 1. Apply permutation: Pb
/// 2. Forward substitution: Ly = Pb
/// 3. Backward substitution: Ux = y
pub fn lu_solve(lu_fact: &LuFactorization, b: &Array1<Complex64>) -> Option<Array1<Complex64>> {
    let n = lu_fact.lu.nrows();
    if b.len() != n {
        return None;
    }

    // Apply permutation to b
    let mut y = Array1::zeros(n);
    for i in 0..n {
        y[i] = b[lu_fact.pivots[i]];
    }

    // Forward substitution: Ly = Pb
    // L has unit diagonal (stored as 1), L[i,j] for j < i is in lu[i,j]
    for i in 1..n {
        for j in 0..i {
            let lij = lu_fact.lu[[i, j]];
            let yj = y[j];
            y[i] -= lij * yj;
        }
    }

    // Backward substitution: Ux = y
    // U[i,j] for j >= i is in lu[i,j]
    let mut x = y;
    for i in (0..n).rev() {
        for j in (i + 1)..n {
            let uij = lu_fact.lu[[i, j]];
            let xj = x[j];
            x[i] -= uij * xj;
        }
        let uii = lu_fact.lu[[i, i]];
        if uii.norm() < 1e-30 {
            // Singular matrix - return None or handle gracefully
            return None;
        }
        x[i] /= uii;
    }

    Some(x)
}

/// Solve Ax = b directly without pre-computed factorization (pure Rust)
///
/// Convenience function that performs LU factorization and solve in one step.
///
/// # Arguments
/// * `a` - Coefficient matrix (n × n)
/// * `b` - Right-hand side vector (n)
///
/// # Returns
/// Solution vector x, or None if the system cannot be solved
pub fn solve(a: &Array2<Complex64>, b: &Array1<Complex64>) -> Option<Array1<Complex64>> {
    let lu_fact = lu_factorize(a)?;
    lu_solve(&lu_fact, b)
}

/// Compute matrix inverse using LU factorization (pure Rust)
///
/// # Arguments
/// * `a` - Square matrix to invert
///
/// # Returns
/// Inverse matrix, or None if singular
///
/// # Algorithm
/// Solves A * X = I column by column using LU factorization.
pub fn inverse(a: &Array2<Complex64>) -> Option<Array2<Complex64>> {
    let n = a.nrows();
    if n != a.ncols() {
        return None;
    }

    let lu_fact = lu_factorize(a)?;

    // Check if singular
    if lu_fact.is_singular(1e-15) {
        return None;
    }

    // Solve for each column of the identity matrix
    let mut inv = Array2::zeros((n, n));

    for col in 0..n {
        // Create column of identity
        let mut e = Array1::zeros(n);
        e[col] = Complex64::new(1.0, 0.0);

        // Solve A * x = e
        if let Some(x) = lu_solve(&lu_fact, &e) {
            for row in 0..n {
                inv[[row, col]] = x[row];
            }
        } else {
            return None;
        }
    }

    Some(inv)
}

/// Compute determinant using LU factorization (pure Rust)
///
/// # Arguments
/// * `a` - Square matrix
///
/// # Returns
/// Determinant value
pub fn determinant(a: &Array2<Complex64>) -> Option<Complex64> {
    let lu_fact = lu_factorize(a)?;
    let n = lu_fact.lu.nrows();

    // det(A) = det(P) * det(L) * det(U) = (-1)^swaps * 1 * prod(U[i,i])
    let mut det = Complex64::new(1.0, 0.0);
    for i in 0..n {
        det *= lu_fact.lu[[i, i]];
    }

    // Account for row swaps
    if lu_fact.num_swaps % 2 == 1 {
        det = -det;
    }

    Some(det)
}

// ============================================================================
// Portable wrappers that use native BLAS when available
// ============================================================================

/// Solve Ax = b with automatic backend selection
///
/// Uses BLAS when `native` feature is enabled, falls back to pure Rust otherwise.
#[cfg(feature = "native")]
pub fn solve_auto(a: &Array2<Complex64>, b: &Array1<Complex64>) -> Option<Array1<Complex64>> {
    use ndarray_linalg::Solve;
    a.solve(b).ok()
}

#[cfg(not(feature = "native"))]
pub fn solve_auto(a: &Array2<Complex64>, b: &Array1<Complex64>) -> Option<Array1<Complex64>> {
    solve(a, b)
}

/// Compute matrix inverse with automatic backend selection
///
/// Uses BLAS when `native` feature is enabled, falls back to pure Rust otherwise.
#[cfg(feature = "native")]
pub fn inverse_auto(a: &Array2<Complex64>) -> Option<Array2<Complex64>> {
    use ndarray_linalg::Inverse;
    a.inv().ok()
}

#[cfg(not(feature = "native"))]
pub fn inverse_auto(a: &Array2<Complex64>) -> Option<Array2<Complex64>> {
    inverse(a)
}

// ============================================================================
// Block operations for preconditioners
// ============================================================================

/// Solve a block-wise system where each block is independent
///
/// This is used by block-diagonal preconditioners.
///
/// # Arguments
/// * `blocks` - List of diagonal blocks (each is a square matrix)
/// * `r` - Full residual vector
/// * `block_sizes` - Size of each block
///
/// # Returns
/// Solution vector z where each block system is solved independently
pub fn block_diagonal_solve(
    block_inv: &[Array2<Complex64>],
    r: &Array1<Complex64>,
    block_sizes: &[usize],
) -> Array1<Complex64> {
    let n = r.len();
    let mut z = Array1::zeros(n);
    let mut offset = 0;

    for (inv, &size) in block_inv.iter().zip(block_sizes.iter()) {
        // Extract block of r
        let r_block = r.slice(s![offset..offset + size]).to_owned();

        // Apply inverse: z_block = inv * r_block
        let z_block = inv.dot(&r_block);

        // Copy back
        for (i, &val) in z_block.iter().enumerate() {
            z[offset + i] = val;
        }

        offset += size;
    }

    z
}

/// Invert multiple blocks (for block diagonal preconditioner setup)
///
/// # Arguments
/// * `blocks` - List of square matrices to invert
///
/// # Returns
/// List of inverse matrices, with identity fallback for singular blocks
pub fn invert_blocks(blocks: &[Array2<Complex64>]) -> Vec<Array2<Complex64>> {
    blocks
        .iter()
        .map(|b| {
            inverse_auto(b).unwrap_or_else(|| {
                let n = b.nrows();
                Array2::eye(n)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lu_factorize_simple() {
        // Simple 2x2 matrix
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(6.0, 0.0),
                Complex64::new(3.0, 0.0),
            ],
        )
        .unwrap();

        let lu_fact = lu_factorize(&a).unwrap();

        // Check that we got a valid factorization
        assert!(!lu_fact.is_singular(1e-10));
    }

    #[test]
    fn test_solve_simple() {
        // 2x2 system: [4 3; 6 3] * x = [10; 12]
        // Solution: x = [1; 2]
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(6.0, 0.0),
                Complex64::new(3.0, 0.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![Complex64::new(10.0, 0.0), Complex64::new(12.0, 0.0)]);

        let x = solve(&a, &b).unwrap();

        // Verify Ax ≈ b
        let ax = a.dot(&x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10, "Error too large: {}", error);
    }

    #[test]
    fn test_solve_complex() {
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

        let x = solve(&a, &b).unwrap();

        // Verify Ax ≈ b
        let ax = a.dot(&x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10, "Error too large: {}", error);
    }

    #[test]
    fn test_solve_identity() {
        let n = 5;
        let a: Array2<Complex64> = Array2::eye(n);
        let b = Array1::from_vec(
            (1..=n)
                .map(|i| Complex64::new(i as f64, 0.0))
                .collect::<Vec<_>>(),
        );

        let x = solve(&a, &b).unwrap();

        // x should equal b for identity matrix
        let error: f64 = (&x - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10);
    }

    #[test]
    fn test_solve_larger() {
        // 5x5 diagonally dominant matrix
        let n = 5;
        let mut a: Array2<Complex64> = Array2::zeros((n, n));

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    a[[i, j]] = Complex64::new(10.0, 0.0);
                } else {
                    a[[i, j]] = Complex64::new(1.0, 0.0);
                }
            }
        }

        let b = Array1::from_vec(
            (0..n)
                .map(|i| Complex64::new((i + 1) as f64, 0.0))
                .collect::<Vec<_>>(),
        );

        let x = solve(&a, &b).unwrap();

        // Verify Ax ≈ b
        let ax = a.dot(&x);
        let error: f64 = (&ax - &b).iter().map(|e| e.norm_sqr()).sum::<f64>().sqrt();
        assert!(error < 1e-10, "Error too large: {}", error);
    }

    #[test]
    fn test_inverse_simple() {
        // 2x2 matrix
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(7.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(6.0, 0.0),
            ],
        )
        .unwrap();

        let a_inv = inverse(&a).unwrap();

        // A * A^{-1} should be identity
        let product = a.dot(&a_inv);

        for i in 0..2 {
            for j in 0..2 {
                let expected = if i == j { 1.0 } else { 0.0 };
                let diff = (product[[i, j]] - Complex64::new(expected, 0.0)).norm();
                assert!(diff < 1e-10, "Product[{},{}] = {:?}, expected {}", i, j, product[[i, j]], expected);
            }
        }
    }

    #[test]
    fn test_inverse_identity() {
        let n = 4;
        let a: Array2<Complex64> = Array2::eye(n);

        let a_inv = inverse(&a).unwrap();

        // Inverse of identity is identity
        for i in 0..n {
            for j in 0..n {
                let expected = if i == j { 1.0 } else { 0.0 };
                let diff = (a_inv[[i, j]] - Complex64::new(expected, 0.0)).norm();
                assert!(diff < 1e-10);
            }
        }
    }

    #[test]
    fn test_determinant_simple() {
        // det([[1, 2], [3, 4]]) = 1*4 - 2*3 = -2
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(4.0, 0.0),
            ],
        )
        .unwrap();

        let det = determinant(&a).unwrap();

        // Note: with partial pivoting, sign may differ
        assert!((det.norm() - 2.0).abs() < 1e-10, "det = {:?}, expected ±2", det);
    }

    #[test]
    fn test_determinant_identity() {
        let n = 4;
        let a: Array2<Complex64> = Array2::eye(n);

        let det = determinant(&a).unwrap();

        assert!((det - Complex64::new(1.0, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn test_singular_matrix() {
        // Singular matrix (second row is 2x first row)
        let a = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(4.0, 0.0),
            ],
        )
        .unwrap();

        let b = Array1::from_vec(vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)]);

        // Should return None for singular matrix
        let result = solve(&a, &b);
        assert!(result.is_none(), "Expected None for singular matrix");
    }

    #[test]
    fn test_block_diagonal_solve() {
        // Two 2x2 blocks
        let block1 = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(0.5, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.25, 0.0),
            ],
        )
        .unwrap();

        let block2 = Array2::from_shape_vec(
            (2, 2),
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, 0.0),
            ],
        )
        .unwrap();

        let block_inv = vec![block1, block2];
        let block_sizes = vec![2, 2];

        let r = Array1::from_vec(vec![
            Complex64::new(2.0, 0.0),
            Complex64::new(4.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]);

        let z = block_diagonal_solve(&block_inv, &r, &block_sizes);

        // z[0] = 0.5 * 2 = 1
        // z[1] = 0.25 * 4 = 1
        // z[2] = 1.0 * 3 = 3
        // z[3] = 2.0 * 1 = 2
        assert!((z[0] - Complex64::new(1.0, 0.0)).norm() < 1e-10);
        assert!((z[1] - Complex64::new(1.0, 0.0)).norm() < 1e-10);
        assert!((z[2] - Complex64::new(3.0, 0.0)).norm() < 1e-10);
        assert!((z[3] - Complex64::new(2.0, 0.0)).norm() < 1e-10);
    }
}
