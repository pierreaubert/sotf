//! Brown test function

use ndarray::Array1;

/// Brown function - ill-conditioned unimodal function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 4]
pub fn brown(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    for i in 0..x.len() - 1 {
        let xi = x[i];
        let xi_plus_1 = x[i + 1];
        sum += (xi.powi(2)).powf(xi_plus_1.powi(2) + 1.0);
        sum += (xi_plus_1.powi(2)).powf(xi.powi(2) + 1.0);
    }
    sum
}
