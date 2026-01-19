//! Alpine N2 test function

use ndarray::Array1;

/// Alpine N.2 function - multimodal with single global minimum
/// Global minimum: f(x) ≈ -2.808^N at x = (2.808, 2.808, ..., 2.808)
/// Bounds: x_i in [0, 10]
pub fn alpine_n2(x: &Array1<f64>) -> f64 {
    -x.iter().map(|&xi| xi.sqrt() * xi.sin()).product::<f64>()
}
