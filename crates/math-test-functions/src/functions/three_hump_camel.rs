//! Three Hump Camel test function

use ndarray::Array1;

/// Three-hump camel function - 2D multimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-5, 5]
pub fn three_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    2.0 * x1.powi(2) - 1.05 * x1.powi(4) + x1.powi(6) / 6.0 + x1 * x2 + x2.powi(2)
}
