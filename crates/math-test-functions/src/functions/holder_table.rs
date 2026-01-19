//! Holder Table test function

use ndarray::Array1;

/// Holder table function - 2D multimodal
/// Global minimum: f(x) = -19.2085 at x = (±8.05502, ±9.66459)
/// Bounds: x_i in [-10, 10]
pub fn holder_table(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let exp_term = (1.0 - (x1.powi(2) + x2.powi(2)).sqrt() / std::f64::consts::PI).abs();
    -(x1 * x2).sin().abs() * exp_term.exp()
}
