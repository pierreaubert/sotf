//! Eggholder test function

use ndarray::Array1;

/// Eggholder function - highly multimodal, very challenging
/// Global minimum: f(x) = -959.6407 at x = (512, 404.2319)
/// Bounds: x_i in [-512, 512]
pub fn eggholder(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -(x2 + 47.0) * (x2 + x1 / 2.0 + 47.0).abs().sqrt().sin()
        - x1 * (x1 - x2 - 47.0).abs().sqrt().sin()
}
