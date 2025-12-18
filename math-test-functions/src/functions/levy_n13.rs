//! Levy N13 test function

use ndarray::Array1;

/// Lévy function N.13 - multimodal function
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-10, 10]
pub fn levy_n13(x: &Array1<f64>) -> f64 {
    let w1 = 1.0 + (x[0] - 1.0) / 4.0;
    let w2 = 1.0 + (x[1] - 1.0) / 4.0;

    (3.0 * std::f64::consts::PI * w1).sin().powi(2)
        + (w1 - 1.0).powi(2) * (1.0 + (3.0 * std::f64::consts::PI * w2).sin().powi(2))
        + (w2 - 1.0).powi(2) * (1.0 + (2.0 * std::f64::consts::PI * w2).sin().powi(2))
}
