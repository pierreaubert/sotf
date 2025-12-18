//! Styblinski Tang2 test function

use ndarray::Array1;

/// Styblinski-Tang function variant (2D specific)
/// Global minimum: f(x) = -78.332 for 2D at x = (-2.903534, -2.903534)
pub fn styblinski_tang2(x: &Array1<f64>) -> f64 {
    let sum: f64 = x
        .iter()
        .map(|&xi| xi.powi(4) - 16.0 * xi.powi(2) + 5.0 * xi)
        .sum();
    sum / 2.0
}
