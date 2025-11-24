//! Sobol initialization
//!
//! Add a Sobol initialization; it should be moved to src-de

/// Generate Sobol quasi-random sequence for initialization
///
/// Uses a simple Sobol sequence implementation for better parameter space coverage
/// than pure random initialization.
///
/// # Arguments
/// * `dimensions` - Number of dimensions (parameters)
/// * `num_samples` - Number of samples to generate
/// * `bounds` - Parameter bounds for scaling
///
/// # Returns
/// Vector of parameter vectors sampled from Sobol sequence
pub fn init_sobol(dimensions: usize, num_samples: usize, bounds: &[(f64, f64)]) -> Vec<Vec<f64>> {
    // Simple Sobol implementation - for production use, consider a more sophisticated library
    let mut samples = Vec::new();

    // Generate quasi-random samples using Van der Corput sequence (simple 1D Sobol)
    for i in 0..num_samples {
        let mut sample = Vec::with_capacity(dimensions);

        for (dim, &(lower, upper)) in bounds.iter().enumerate().take(dimensions) {
            // Van der Corput sequence in base 2 for dimension 0, base 3 for dim 1, etc.
            let base = match dim {
                0 => 2,
                1 => 3,
                2 => 5,
                3 => 7,
                4 => 11,
                _ => 2 + (dim % 10), // Simple fallback
            };

            let quasi_random = van_der_corput(i + 1, base);

            // Scale to bounds
            let scaled = lower + quasi_random * (upper - lower);
            sample.push(scaled);
        }

        samples.push(sample);
    }

    samples
}

/// Van der Corput sequence for quasi-random number generation
fn van_der_corput(mut n: usize, base: usize) -> f64 {
    let mut result = 0.0;
    let mut f = 1.0 / base as f64;

    while n > 0 {
        result += (n % base) as f64 * f;
        n /= base;
        f /= base as f64;
    }

    result
}

#[cfg(test)]
mod init_sobol_tests {
    use super::*;

    #[test]
    fn test_van_der_corput() {
        // Test basic Van der Corput sequence properties
        let val1 = van_der_corput(1, 2);
        let val2 = van_der_corput(2, 2);
        let val3 = van_der_corput(3, 2);

        // Should be in [0, 1)
        assert!(val1 >= 0.0 && val1 < 1.0);
        assert!(val2 >= 0.0 && val2 < 1.0);
        assert!(val3 >= 0.0 && val3 < 1.0);

        // Should be different values
        assert_ne!(val1, val2);
        assert_ne!(val2, val3);
    }

    #[test]
    fn test_init_sobol() {
        let bounds = vec![(0.0, 10.0), (0.1, 5.0), (-12.0, 12.0)];
        let samples = init_sobol(3, 5, &bounds);

        assert_eq!(samples.len(), 5);
        for sample in &samples {
            assert_eq!(sample.len(), 3);
            // Check bounds
            assert!(sample[0] >= 0.0 && sample[0] <= 10.0);
            assert!(sample[1] >= 0.1 && sample[1] <= 5.0);
            assert!(sample[2] >= -12.0 && sample[2] <= 12.0);
        }
    }
}
