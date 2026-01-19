//! Dixons Price test function

use ndarray::Array1;

/// Dixon's Price function - unimodal, non-separable
/// Global minimum: f(x) = 0 at x = (1, 2^(-1/2), 2^(-2/2), ..., 2^(-(i-1)/2))
/// Bounds: x_i in [-10, 10]
pub fn dixons_price(x: &Array1<f64>) -> f64 {
    let first_term = (x[0] - 1.0).powi(2);
    let sum_term: f64 = x
        .iter()
        .skip(1)
        .enumerate()
        .map(|(i, &xi)| (i + 2) as f64 * (2.0 * xi.powi(2) - x[i]).powi(2))
        .sum();
    first_term + sum_term
}
