//! Freudenstein Roth test function

use ndarray::Array1;

/// Freudenstein and Roth function - multimodal with ill-conditioning
/// Global minimum: f(x) = 0 at x = (5, 4)
/// Bounds: x_i in [-10, 10]
pub fn freudenstein_roth(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (-13.0 + x1 + ((5.0 - x2) * x2 - 2.0) * x2).powi(2)
        + (-29.0 + x1 + ((x2 + 1.0) * x2 - 14.0) * x2).powi(2)
}
