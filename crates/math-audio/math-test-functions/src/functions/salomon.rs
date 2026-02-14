//! Salomon test function

use ndarray::Array1;

/// Salomon function - multimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn salomon(x: &Array1<f64>) -> f64 {
    let norm = x.iter().map(|&xi| xi.powi(2)).sum::<f64>().sqrt();
    1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm
}
