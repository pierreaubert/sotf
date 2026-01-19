//! Power Sum test function

use ndarray::Array1;

/// Power Sum Function - constrained optimization problem
/// Global minimum: complex, depends on parameters b
/// Bounds: x_i in [0, d] where d is dimension
pub fn power_sum(x: &Array1<f64>) -> f64 {
    let b = [8.0, 18.0, 44.0, 114.0]; // Parameters for up to 4D
    let d = x.len().min(4);

    let mut sum = 0.0;
    for i in 1..=d {
        let power_sum: f64 = x.iter().take(d).map(|&xj| xj.powf(i as f64)).sum();
        sum += (power_sum - b[i - 1]).powi(2);
    }
    sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_sum_known_properties() {
        use crate::{FunctionMetadata, get_function_metadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("power_sum")
            .expect("Function power_sum should have metadata");

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
            let actual_value = power_sum(&x);

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
            let result = power_sum(&x);

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
