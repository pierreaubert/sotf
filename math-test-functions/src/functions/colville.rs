//! Colville test function

use ndarray::Array1;

/// Colville function - multimodal, non-separable
/// Global minimum: f(x) = 0 at x = (1, 1, 1, 1)
/// Bounds: x_i in [-10, 10]
pub fn colville(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let x3 = if x.len() > 2 { x[2] } else { 1.0 };
    let x4 = if x.len() > 3 { x[3] } else { 1.0 };

    100.0 * (x1.powi(2) - x2).powi(2)
        + (x1 - 1.0).powi(2)
        + (x3 - 1.0).powi(2)
        + 90.0 * (x3.powi(2) - x4).powi(2)
        + 10.1 * ((x2 - 1.0).powi(2) + (x4 - 1.0).powi(2))
        + 19.8 * (x2 - 1.0) * (x4 - 1.0)
}
