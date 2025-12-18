//! Sum Of Different Powers test function

use ndarray::Array1;

/// Sum of different powers function - unimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 1]
pub fn sum_of_different_powers(x: &Array1<f64>) -> f64 {
    x.iter()
        .enumerate()
        .map(|(i, &xi)| xi.abs().powf(i as f64 + 2.0))
        .sum::<f64>()
}
