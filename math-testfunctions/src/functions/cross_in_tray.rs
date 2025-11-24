//! Cross In Tray test function

use ndarray::Array1;

/// Cross-in-tray function - 2D multimodal function
/// Global minimum: f(x) = -2.06261 at x = (±1.34941, ±1.34941)
/// Bounds: x_i in [-10, 10]
pub fn cross_in_tray(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let exp_term = (100.0 - (x1.powi(2) + x2.powi(2)).sqrt() / std::f64::consts::PI).abs();
    -0.0001 * ((x1 * x2).sin().abs() * exp_term.exp() + 1.0).powf(0.1)
}
