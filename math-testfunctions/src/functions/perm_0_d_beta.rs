//! Perm 0 D Beta test function

use ndarray::Array1;

/// Perm Function 0, d, β - bowl-shaped function
/// f(x) = ∑_{i=1}^d [∑_{j=1}^d (j+β)(x_j^i - (1/j)^i)]^2
/// Global minimum: f(x) = 0 at x = (1, 1/2, 1/3, ..., 1/d)
/// Bounds: x_i in [-1, 1]
pub fn perm_0_d_beta(x: &Array1<f64>) -> f64 {
    let d = x.len();
    let beta = 0.5; // Parameter β (smaller value for numerical stability)

    let mut outer_sum = 0.0;
    for i in 1..=d {
        let mut inner_sum = 0.0;
        for j in 1..=d {
            let xj = x[j - 1];
            let target = (1.0 / j as f64).powf(i as f64);
            inner_sum += (j as f64 + beta) * (xj.powf(i as f64) - target);
        }
        outer_sum += inner_sum.powi(2);
    }
    outer_sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perm_0_d_beta_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("perm_0_d_beta")
            .expect("Function perm_0_d_beta should have metadata");

        // Test 1: Verify global minima are within bounds
        for (minimum_coords, expected_value) in &meta.global_minima {
            assert!(
                minimum_coords.len() >= meta.bounds.len() || meta.bounds.len() == 1,
                "Global minimum coordinates should match bounds dimensions"
            );

            for (i, &coord) in minimum_coords.iter().enumerate() {
                if i < meta.bounds.len() {
                    let (lower, upper) = meta.bounds[i];
                    assert!(
                        coord >= lower && coord <= upper,
                        "Global minimum coordinate {} = {} should be within bounds [{} {}]",
                        i,
                        coord,
                        lower,
                        upper
                    );
                }
            }
        }

        // Test 2: Verify function evaluates to expected values at global minima
        let tolerance = 1e-6; // Reasonable tolerance for numerical precision
        for (minimum_coords, expected_value) in &meta.global_minima {
            let x = Array1::from_vec(minimum_coords.clone());
            let actual_value = perm_0_d_beta(&x);

            let error = (actual_value - expected_value).abs();
            assert!(
                error <= tolerance,
                "Function value at global minimum {:?} should be {}, got {}, error: {}",
                minimum_coords,
                expected_value,
                actual_value,
                error
            );
        }

        // Test 3: Basic function properties
        if !meta.global_minima.is_empty() {
            let (first_minimum, _) = &meta.global_minima[0];
            let x = Array1::from_vec(first_minimum.clone());
            let result = perm_0_d_beta(&x);

            assert!(
                result.is_finite(),
                "Function should return finite values at global minimum"
            );
            assert!(
                !result.is_nan(),
                "Function should not return NaN at global minimum"
            );
        }
    }
}
