//! Salomon Corrected test function

use ndarray::Array1;

/// Salomon function (corrected implementation)
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn salomon_corrected(x: &Array1<f64>) -> f64 {
    let norm = x.iter().map(|&xi| xi.powi(2)).sum::<f64>().sqrt();
    if norm == 0.0 {
        0.0
    } else {
        1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm
    }
}
