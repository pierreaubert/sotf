//! Drop Wave test function

use ndarray::Array1;

/// Drop wave function - 2D multimodal
/// Global minimum: f(x) = -1.0 at x = (0, 0)
/// Bounds: x_i in [-5.12, 5.12]
pub fn drop_wave(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let numerator = 1.0 + (12.0 * (x1.powi(2) + x2.powi(2)).sqrt()).cos();
    let denominator = 0.5 * (x1.powi(2) + x2.powi(2)) + 2.0;
    -numerator / denominator
}
