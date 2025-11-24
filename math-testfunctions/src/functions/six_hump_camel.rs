//! Six Hump Camel test function

use ndarray::Array1;

/// Six-hump camel function - 2D multimodal
/// Global minimum: f(x) = -1.0316 at x = (0.0898, -0.7126) and (-0.0898, 0.7126)
/// Bounds: x1 in [-3, 3], x2 in [-2, 2]
pub fn six_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (4.0 - 2.1 * x1.powi(2) + x1.powi(4) / 3.0) * x1.powi(2)
        + x1 * x2
        + (-4.0 + 4.0 * x2.powi(2)) * x2.powi(2)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_six_hump_camel_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("six_hump_camel")
            .expect("Function six_hump_camel should have metadata");

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
            let actual_value = six_hump_camel(&x);

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
            let result = six_hump_camel(&x);

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
