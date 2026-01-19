//! Rosenbrock Disk Constraint test function

use ndarray::Array1;

/// Rosenbrock disk constraint: x^2 + y^2 <= 2
pub fn rosenbrock_disk_constraint(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + x[1].powi(2) - 2.0
}
