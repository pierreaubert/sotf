//! Easom test function

use ndarray::Array1;

/// Easom function - multimodal with very narrow global basin
/// Global minimum: f(x) = -1 at x = (π, π)
/// Bounds: x_i in [-100, 100]
pub fn easom(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -x1.cos()
        * x2.cos()
        * (-(x1 - std::f64::consts::PI).powi(2) - (x2 - std::f64::consts::PI).powi(2)).exp()
}
