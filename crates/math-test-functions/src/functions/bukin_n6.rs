//! Bukin N6 test function

use ndarray::Array1;

/// Bukin N.6 function - highly multimodal with narrow global optimum
/// Global minimum: f(x) = 0 at x = (-10, 1)
/// Bounds: x1 in [-15, -5], x2 in [-3, 3]
pub fn bukin_n6(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    100.0 * (x2 - 0.01 * x1.powi(2)).abs().sqrt() + 0.01 * (x1 + 10.0).abs()
}
