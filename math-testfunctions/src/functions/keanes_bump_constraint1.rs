//! Keanes Bump Constraint1 test function

use ndarray::Array1;

/// First constraint for Keane's bump function: x1*x2*x3*x4 >= 0.75
/// Returns violation amount (0 if satisfied, positive if violated)
pub fn keanes_bump_constraint1(x: &Array1<f64>) -> f64 {
    let product: f64 = x.iter().take(4).product();
    0.75 - product // Constraint: product >= 0.75, so violation is 0.75 - product
}
