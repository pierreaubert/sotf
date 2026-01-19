//! Zakharov test function

use ndarray::Array1;

/// Zakharov function - unimodal quadratic function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-5, 10]
pub fn zakharov(x: &Array1<f64>) -> f64 {
    let sum1: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum2: f64 = x
        .iter()
        .enumerate()
        .map(|(i, &xi)| 0.5 * (i + 1) as f64 * xi)
        .sum();
    sum1 + sum2.powi(2) + sum2.powi(4)
}
