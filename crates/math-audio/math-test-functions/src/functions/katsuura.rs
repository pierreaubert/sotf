//! Katsuura test function

use ndarray::Array1;

/// Katsuura function - fractal-like multimodal function
/// Global minimum: f(x) = 1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [0, 100]
pub fn katsuura(x: &Array1<f64>) -> f64 {
    let d = x.len();
    let mut product = 1.0;

    for (i, &xi) in x.iter().enumerate() {
        let mut sum = 0.0;
        // Limit j to prevent overflow, 20 is sufficient for precision
        for j in 1..=20 {
            let power2j = (2.0_f64).powi(j);
            let term = (power2j * xi).abs() - (power2j * xi).round().abs();
            sum += term / power2j;
        }
        product *= 1.0 + (i + 1) as f64 * sum;
    }

    let factor = 100.0 / (d as f64).powi(2);
    factor * product - factor
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_katsuura_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("katsuura")
            .expect("Function katsuura should have metadata");

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
            let actual_value = katsuura(&x);

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
            let result = katsuura(&x);

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
