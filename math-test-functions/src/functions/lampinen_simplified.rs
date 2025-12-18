//! Lampinen Simplified test function

use ndarray::Array1;

/// Simplified Lampinen test problem (unconstrained version)
/// f(x) = sum(5*x\[i\]) - sum(x\[i\]^2) for i in 0..4, - sum(x\[j\]) for j in 4..
pub fn lampinen_simplified(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;

    // First 4 variables: 5*x[i] - x[i]^2
    for i in 0..4.min(x.len()) {
        sum += 5.0 * x[i] - x[i] * x[i];
    }

    // Remaining variables: -x[j]
    for i in 4..x.len() {
        sum -= x[i];
    }

    -sum // Minimize negative (i.e., maximize original)
}
