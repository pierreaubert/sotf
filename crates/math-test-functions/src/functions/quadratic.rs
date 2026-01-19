//! Quadratic test function

use ndarray::Array1;

/// Simple quadratic function for basic testing
/// f(x) = sum(x\[i\]^2)
/// Global minimum at (0, 0, ..., 0) with f = 0
pub fn quadratic(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| xi * xi).sum()
}
