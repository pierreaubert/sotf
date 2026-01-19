//! Hartman 4D test function

use ndarray::Array1;

/// Hartman 4-D function - 4D multimodal with 4 local minima
/// Global minimum: f(x) ≈ -3.72983 at x ≈ [0.1873, 0.1936, 0.5576, 0.2647]
/// Bounds: x_i in [0, 1]
/// Reference: Hartman, J.K. (1973). Some experiments in global optimization
pub fn hartman_4d(x: &Array1<f64>) -> f64 {
    // Original Hartmann 4-D parameters from literature
    let a = [
        [10.0, 3.0, 17.0, 3.5],
        [0.05, 10.0, 17.0, 0.1],
        [3.0, 3.5, 1.7, 10.0],
        [17.0, 8.0, 0.05, 10.0],
    ];
    let c = [1.0, 1.2, 3.0, 3.2];
    let p = [
        [0.1312, 0.1696, 0.5569, 0.0124],
        [0.2329, 0.4135, 0.8307, 0.3736],
        [0.2348, 0.1451, 0.3522, 0.2883],
        [0.4047, 0.8828, 0.8732, 0.5743],
    ];

    -c.iter()
        .enumerate()
        .map(|(i, &ci)| {
            let inner_sum = a[i]
                .iter()
                .zip(p[i].iter())
                .enumerate()
                .map(|(j, (&aij, &pij))| aij * (x[j] - pij).powi(2))
                .sum::<f64>();
            ci * (-inner_sum).exp()
        })
        .sum::<f64>()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hartman_4d_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("hartman_4d")
            .expect("Function hartman_4d should have metadata");

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
        for (minimum_coords, expected_value) in &meta.global_minima {
            let x = Array1::from_vec(minimum_coords.clone());
            let actual_value = hartman_4d(&x);

            let error = (actual_value - expected_value).abs();
            // Use adaptive tolerance based on magnitude of expected value
            let tolerance = if expected_value.abs() > 1.0 {
                1e-4 * expected_value.abs() // Relative tolerance for large values
            } else {
                1e-6 // Absolute tolerance for small values
            };

            assert!(
                error <= tolerance,
                "Function value at global minimum {:?} should be {}, got {}, error: {} (tolerance: {})",
                minimum_coords,
                expected_value,
                actual_value,
                error,
                tolerance
            );
        }

        // Test 3: Basic function properties
        if !meta.global_minima.is_empty() {
            let (first_minimum, _) = &meta.global_minima[0];
            let x = Array1::from_vec(first_minimum.clone());
            let result = hartman_4d(&x);

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
