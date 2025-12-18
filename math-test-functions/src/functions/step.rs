//! Step test function

use ndarray::Array1;

/// Step function - discontinuous, multimodal
/// Global minimum: f(x) = 0 at x = (0.5, 0.5, ..., 0.5)
/// Bounds: x_i in [-100, 100]
pub fn step(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| (xi + 0.5).floor().powi(2)).sum::<f64>()
}
