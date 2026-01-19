//! Branin test function

use ndarray::Array1;

/// Branin function - multimodal, 2D only
/// Global minimum: f(x) = 0.397887 at x = (-π, 12.275), (π, 2.275), (9.42478, 2.475)
/// Bounds: x1 in [-5, 10], x2 in [0, 15]
pub fn branin(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let a = 1.0;
    let b = 5.1 / (4.0 * std::f64::consts::PI.powi(2));
    let c = 5.0 / std::f64::consts::PI;
    let r = 6.0;
    let s = 10.0;
    let t = 1.0 / (8.0 * std::f64::consts::PI);

    a * (x2 - b * x1.powi(2) + c * x1 - r).powi(2) + s * (1.0 - t) * x1.cos() + s
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branin_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("branin")
            .expect("Function branin should have metadata");

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
            let actual_value = branin(&x);

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
            let result = branin(&x);

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
