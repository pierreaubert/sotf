//! Shekel test function

use ndarray::Array1;

/// Shekel Function - multimodal function with m local minima
/// Global minimum depends on m parameter
/// Bounds: x_i in [0, 10]
pub fn shekel(x: &Array1<f64>) -> f64 {
    let m = 10; // Number of local minima
    let a = [
        [4.0, 4.0, 4.0, 4.0],
        [1.0, 1.0, 1.0, 1.0],
        [8.0, 8.0, 8.0, 8.0],
        [6.0, 6.0, 6.0, 6.0],
        [3.0, 7.0, 3.0, 7.0],
        [2.0, 9.0, 2.0, 9.0],
        [5.0, 5.0, 3.0, 3.0],
        [8.0, 1.0, 8.0, 1.0],
        [6.0, 2.0, 6.0, 2.0],
        [7.0, 3.6, 7.0, 3.6],
    ];
    let c = [0.1, 0.2, 0.2, 0.4, 0.4, 0.6, 0.3, 0.7, 0.5, 0.5];

    let mut sum = 0.0;
    for i in 0..m.min(10) {
        let mut inner_sum = 0.0;
        for j in 0..4.min(x.len()) {
            inner_sum += (x[j] - a[i][j]).powi(2);
        }
        sum += 1.0 / (inner_sum + c[i]);
    }
    -sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shekel_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("shekel")
            .expect("Function shekel should have metadata");

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
            let actual_value = shekel(&x);

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
            let result = shekel(&x);

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
