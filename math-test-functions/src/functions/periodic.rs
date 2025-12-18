//! Periodic test function

use ndarray::Array1;

/// Periodic function - multimodal with periodic landscape
/// Global minimum: f(x) = 0.9 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn periodic(x: &Array1<f64>) -> f64 {
    let sum_sin_squared: f64 = x.iter().map(|&xi| xi.sin().powi(2)).sum();
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();

    1.0 + sum_sin_squared - 0.1 * (-sum_squares).exp()
}
