//! Cosine Mixture test function

use ndarray::Array1;

/// Cosine mixture function - multimodal
/// Global minimum depends on dimension
/// Bounds: x_i in [-1, 1]
pub fn cosine_mixture(x: &Array1<f64>) -> f64 {
    let sum_cos = x
        .iter()
        .map(|&xi| (5.0 * std::f64::consts::PI * xi).cos())
        .sum::<f64>();
    let sum_sq = x.iter().map(|&xi| xi.powi(2)).sum::<f64>();
    -0.1 * sum_cos + sum_sq
}
