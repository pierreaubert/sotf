//! Keanes Bump Objective test function

use ndarray::Array1;

/// Keane's bump function objective (for constrained optimization)
/// Subject to constraints: x1*x2*x3*x4 >= 0.75 and sum(x_i) <= 7.5*n
/// Bounds: x_i in [0, 10]
pub fn keanes_bump_objective(x: &Array1<f64>) -> f64 {
    let sum_cos4: f64 = x.iter().map(|&xi| xi.cos().powi(4)).sum();
    let prod_cos2: f64 = x.iter().map(|&xi| xi.cos().powi(2)).product();
    let sum_i_xi2: f64 = x
        .iter()
        .enumerate()
        .map(|(i, &xi)| (i + 1) as f64 * xi.powi(2))
        .sum();

    let denom = sum_i_xi2.sqrt().max(1.0e-12);
    -(sum_cos4 - 2.0 * prod_cos2).abs() / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_finite_for_standard_2d_and_4d_cases() {
        assert!(keanes_bump_objective(&Array1::from(vec![0.0, 0.0])).is_finite());
        assert!(keanes_bump_objective(&Array1::from(vec![0.0; 4])).is_finite());
    }

    #[test]
    fn known_2d_minimum_matches_metadata() {
        let x = Array1::from(vec![1.393249, 0.0]);
        let value = keanes_bump_objective(&x);
        assert!((value - -0.673668).abs() < 1e-5, "got {value}");
    }
}
