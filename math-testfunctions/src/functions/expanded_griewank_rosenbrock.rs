//! Expanded Griewank Rosenbrock test function

use ndarray::Array1;

/// Expanded Griewank plus Rosenbrock function (F8F2)
/// Combines characteristics of both functions
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-5, 5]
pub fn expanded_griewank_rosenbrock(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    let n = x.len();

    for i in 0..n {
        let xi = x[i];
        let xi_plus_1 = x[(i + 1) % n]; // Wrap around for the last element

        // Rosenbrock component
        let rosenbrock = 100.0 * (xi.powi(2) - xi_plus_1).powi(2) + (xi - 1.0).powi(2);

        // Griewank transformation of Rosenbrock
        let griewank_part = rosenbrock / 4000.0 - (rosenbrock).cos() + 1.0;

        sum += griewank_part;
    }

    sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expanded_griewank_rosenbrock_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("expanded_griewank_rosenbrock")
            .expect("Function expanded_griewank_rosenbrock should have metadata");

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
            let actual_value = expanded_griewank_rosenbrock(&x);

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
            let result = expanded_griewank_rosenbrock(&x);

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
