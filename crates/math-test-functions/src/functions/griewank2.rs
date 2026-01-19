//! Griewank2 test function

use ndarray::Array1;

/// Griewank2 function - variant of Griewank with different scaling
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-600, 600]
pub fn griewank2(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let product_cos: f64 = x
        .iter()
        .enumerate()
        .map(|(i, &xi)| (xi / ((i + 1) as f64).sqrt()).cos())
        .product();
    sum_squares / 4000.0 - product_cos + 1.0
}
