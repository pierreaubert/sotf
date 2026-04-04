/// Generate Sobol quasi-random sequence for initialization.
///
/// Uses Van der Corput sequences in co-prime bases for better parameter space
/// coverage than pure random initialization.
///
/// # Arguments
/// * `dimensions` - Number of dimensions (parameters)
/// * `num_samples` - Number of samples to generate
/// * `bounds` - Parameter bounds for scaling
///
/// # Returns
/// Vector of parameter vectors sampled from the quasi-random sequence
pub fn init_sobol(dimensions: usize, num_samples: usize, bounds: &[(f64, f64)]) -> Vec<Vec<f64>> {
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let mut sample = Vec::with_capacity(dimensions);

        for (dim, &(lower, upper)) in bounds.iter().enumerate().take(dimensions) {
            let base = match dim {
                0 => 2,
                1 => 3,
                2 => 5,
                3 => 7,
                4 => 11,
                _ => 2 + (dim % 10),
            };

            let quasi_random = van_der_corput(i + 1, base);
            sample.push(lower + quasi_random * (upper - lower));
        }

        samples.push(sample);
    }

    samples
}

/// Van der Corput sequence for quasi-random number generation.
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
mod tests {
    use super::*;

    #[test]
    fn test_van_der_corput() {
        let val1 = van_der_corput(1, 2);
        let val2 = van_der_corput(2, 2);
        let val3 = van_der_corput(3, 2);

        assert!((0.0..1.0).contains(&val1));
        assert!((0.0..1.0).contains(&val2));
        assert!((0.0..1.0).contains(&val3));

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
            assert!((0.0..=10.0).contains(&sample[0]));
            assert!((0.1..=5.0).contains(&sample[1]));
            assert!((-12.0..=12.0).contains(&sample[2]));
        }
    }
}
