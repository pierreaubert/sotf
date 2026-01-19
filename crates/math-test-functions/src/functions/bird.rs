//! Bird test function

use ndarray::Array1;

/// Bird function - 2D multimodal
/// Global minimum: f(x) = -106.764537 at x = (4.70104, 3.15294) and (-1.58214, -3.13024)
/// Bounds: x_i in [-2π, 2π]
pub fn bird(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.sin() * (x2 - 15.0).exp() + (x1 - x2.cos()).powi(2)
}
