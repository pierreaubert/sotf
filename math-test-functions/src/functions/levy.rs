//! Levy test function

use ndarray::Array1;

/// Levy function - multimodal function (generalized version)
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-10, 10]
pub fn levy(x: &Array1<f64>) -> f64 {
    use std::f64::consts::PI;

    let w: Vec<f64> = x.iter().map(|&xi| 1.0 + (xi - 1.0) / 4.0).collect();

    let first_term = (PI * w[0]).sin().powi(2);

    let middle_sum: f64 = w
        .iter()
        .take(w.len() - 1)
        .map(|&wi| (wi - 1.0).powi(2) * (1.0 + 10.0 * (PI * wi + 1.0).sin().powi(2)))
        .sum();

    let last_term = {
        let wn = w[w.len() - 1];
        (wn - 1.0).powi(2) * (1.0 + (2.0 * PI * wn).sin().powi(2))
    };

    first_term + middle_sum + last_term
}
