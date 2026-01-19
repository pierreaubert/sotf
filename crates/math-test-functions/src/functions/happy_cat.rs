//! Happy Cat test function

use ndarray::Array1;

/// Happy Cat function - recent benchmark with interesting landscape
/// Global minimum: f(x) = 0 at x = (±1, ±1, ..., ±1)
/// Bounds: x_i in [-2, 2]
pub fn happy_cat(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum_x: f64 = x.iter().sum();

    ((sum_squares - n).powi(2)).powf(0.25) + (0.5 * sum_squares + sum_x) / n + 0.5
}
