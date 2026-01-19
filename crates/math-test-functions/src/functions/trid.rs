//! Trid test function

use ndarray::Array1;

/// Trid function - unimodal, bowl-shaped
/// Global minimum for 2D: f(x) = -2 at x = (2, 2)
/// Bounds: x_i in [-d^2, d^2] where d is dimension
pub fn trid(x: &Array1<f64>) -> f64 {
    let sum1 = x.iter().map(|&xi| (xi - 1.0).powi(2)).sum::<f64>();
    let sum2 = x.windows(2).into_iter().map(|w| w[0] * w[1]).sum::<f64>();
    sum1 - sum2
}
