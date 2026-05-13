/// Generate Halton quasi-random sequence for initialization.
///
/// Uses Van der Corput sequences in the first `dimensions` prime bases.
/// This provides better parameter space coverage than pure random
/// initialization and avoids the correlated-base bug of the old fallback.
///
/// # Arguments
/// * `dimensions` - Number of dimensions (parameters)
/// * `num_samples` - Number of samples to generate
/// * `bounds` - Parameter bounds for scaling
///
/// # Returns
/// Vector of parameter vectors sampled from the quasi-random sequence
pub fn init_halton(dimensions: usize, num_samples: usize, bounds: &[(f64, f64)]) -> Vec<Vec<f64>> {
    let bases = halton_bases(dimensions);
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let mut sample = Vec::with_capacity(dimensions);

        for (dim, &(lower, upper)) in bounds.iter().enumerate().take(dimensions) {
            let quasi_random = van_der_corput(i + 1, bases[dim]);
            sample.push(lower + quasi_random * (upper - lower));
        }

        samples.push(sample);
    }

    samples
}

/// Deprecated alias for [`init_halton`].
/// The old name claimed a Sobol sequence, but the implementation was a
/// Halton-style Van der Corput construction.
#[deprecated(since = "0.5.9", note = "Use init_halton instead")]
pub fn init_sobol(dimensions: usize, num_samples: usize, bounds: &[(f64, f64)]) -> Vec<Vec<f64>> {
    init_halton(dimensions, num_samples, bounds)
}

/// Return the first `n` prime numbers to use as Halton bases.
fn halton_bases(n: usize) -> Vec<usize> {
    let mut primes = Vec::with_capacity(n);
    let mut candidate = 2;
    while primes.len() < n {
        if is_prime(candidate) {
            primes.push(candidate);
        }
        candidate += 1;
    }
    primes
}

fn is_prime(n: usize) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 {
        return true;
    }
    if n.is_multiple_of(2) {
        return false;
    }
    let mut i = 3;
    while i * i <= n {
        if n.is_multiple_of(i) {
            return false;
        }
        i += 2;
    }
    true
}

#[cfg(test)]
fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
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
    fn test_init_halton() {
        let bounds = vec![(0.0, 10.0), (0.1, 5.0), (-12.0, 12.0)];
        let samples = init_halton(3, 5, &bounds);

        assert_eq!(samples.len(), 5);
        for sample in &samples {
            assert_eq!(sample.len(), 3);
            assert!((0.0..=10.0).contains(&sample[0]));
            assert!((0.1..=5.0).contains(&sample[1]));
            assert!((-12.0..=12.0).contains(&sample[2]));
        }
    }

    #[test]
    fn test_halton_bases_are_coprime() {
        // The old fallback used bases like 2 + (dim % 10), which are not coprime
        // (e.g. dim 8 used base 10, dim 10 used base 2 again). A proper Halton
        // sequence uses the first n primes, which are pairwise coprime.
        let bases = halton_bases(20);
        for i in 0..bases.len() {
            for j in (i + 1)..bases.len() {
                assert_eq!(
                    gcd(bases[i], bases[j]),
                    1,
                    "bases {} and {} must be coprime",
                    bases[i],
                    bases[j]
                );
            }
        }
    }

    #[test]
    fn test_halton_no_cycling() {
        // With the old dim%10 cycling, dim 10 reused base 2, causing correlation.
        // Halton uses unique primes for each dimension.
        let bases = halton_bases(15);
        let unique: std::collections::HashSet<_> = bases.iter().cloned().collect();
        assert_eq!(
            unique.len(),
            bases.len(),
            "each dimension must have a unique base"
        );
    }
}
