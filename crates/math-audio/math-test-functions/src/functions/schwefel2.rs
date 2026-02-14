//! Schwefel2 test function

use ndarray::Array1;

/// Schwefel function variant (different from the main schwefel)
pub fn schwefel2(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let sum: f64 = x
        .iter()
        .enumerate()
        .map(|(i, &xi)| {
            let inner_sum: f64 = x.iter().take(i + 1).copied().sum();
            inner_sum.powi(2)
        })
        .sum();
    sum
}
