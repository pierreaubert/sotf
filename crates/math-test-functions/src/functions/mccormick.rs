//! Mccormick test function

use ndarray::Array1;

/// McCormick function - 2D function
/// Global minimum: f(x) = -1.9133 at x = (-0.54719, -1.54719)
/// Bounds: x1 in [-1.5, 4], x2 in [-3, 4]
pub fn mccormick(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (x1 + x2).sin() + (x1 - x2).powi(2) - 1.5 * x1 + 2.5 * x2 + 1.0
}
