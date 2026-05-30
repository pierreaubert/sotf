//! Keanes Bump Constraint1 test function

use ndarray::Array1;

/// First constraint for Keane's bump function: x1*x2*x3*x4 >= 0.75
/// Returns violation amount (0 if satisfied, positive if violated)
pub fn keanes_bump_constraint1(x: &Array1<f64>) -> f64 {
    let product: f64 = (0..4).map(|i| x.get(i).copied().unwrap_or(0.0)).product();
    0.75 - product // Constraint: product >= 0.75, so violation is 0.75 - product
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn satisfied_when_first_four_product_is_large_enough() {
        let x = Array1::from(vec![1.0, 1.0, 1.0, 1.0]);
        assert!(keanes_bump_constraint1(&x) <= 0.0);
    }

    #[test]
    fn short_inputs_are_treated_as_missing_zero_dimensions() {
        let x = Array1::from(vec![1.0, 1.0]);
        assert_eq!(keanes_bump_constraint1(&x), 0.75);
    }
}
