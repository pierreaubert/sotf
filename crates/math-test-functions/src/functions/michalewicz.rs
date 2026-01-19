//! Michalewicz test function

use ndarray::Array1;

/// Michalewicz function - N-dimensional multimodal
/// Global minimum depends on dimension (e.g., -1.8013 for 2D, -9.66 for 10D)
/// Bounds: x_i in [0, π]
pub fn michalewicz(x: &Array1<f64>) -> f64 {
    let m = 10.0; // Steepness parameter
    -x.iter()
        .enumerate()
        .map(|(i, &xi)| {
            xi.sin()
                * ((i as f64 + 1.0) * xi.powi(2) / std::f64::consts::PI)
                    .sin()
                    .powf(2.0 * m)
        })
        .sum::<f64>()
}
