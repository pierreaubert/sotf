//! Keanes Bump Constraint2 test function

use ndarray::Array1;

/// Second constraint for Keane's bump function: sum(x_i) <= 7.5*n
/// Returns violation amount (0 if satisfied, positive if violated)
pub fn keanes_bump_constraint2(x: &Array1<f64>) -> f64 {
    let sum: f64 = x.iter().sum();
    let limit = 7.5 * x.len() as f64;
    sum - limit // Constraint: sum <= limit, so violation is sum - limit
}
