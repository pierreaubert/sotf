//! Bohachevsky1 test function

use ndarray::Array1;

/// Bohachevsky function 1 - 2D multimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-100, 100]
pub fn bohachevsky1(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.powi(2) + 2.0 * x2.powi(2)
        - 0.3 * (3.0 * std::f64::consts::PI * x1).cos()
        - 0.4 * (4.0 * std::f64::consts::PI * x2).cos()
        + 0.7
}
