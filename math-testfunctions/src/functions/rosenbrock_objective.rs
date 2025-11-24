//! Rosenbrock Objective test function

use ndarray::Array1;

/// Rosenbrock objective function (2D)
pub fn rosenbrock_objective(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    100.0 * (x2 - x1.powi(2)).powi(2) + (1.0 - x1).powi(2)
}
