//! Shubert test function

use ndarray::Array1;

/// Shubert function - highly multimodal with many global minima
/// Global minimum: f(x) = -186.7309 (2D), multiple locations
/// Bounds: x_i in [-10, 10]
pub fn shubert(x: &Array1<f64>) -> f64 {
    x.iter()
        .map(|&xi| {
            (1..=5)
                .map(|i| {
                    let i_f64 = i as f64;
                    i_f64 * ((i_f64 + 1.0) * xi + i_f64).cos()
                })
                .sum::<f64>()
        })
        .product()
}
