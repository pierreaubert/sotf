//! Crossover monotonicity constraint
//!
//! Ensures that crossover frequencies are strictly monotonically increasing.
//! For multi-driver systems, we have parameters: [gains, log10(xover_freqs)]
//! This constraint ensures: log10(xover_i) < log10(xover_i+1) for all i

/// Data required for crossover monotonicity constraint
#[derive(Debug, Clone)]
pub struct CrossoverMonotonicityConstraintData {
    /// Number of drivers
    pub n_drivers: usize,
    /// Minimum separation in log10 space (e.g., 0.1 = ~26% frequency separation)
    pub min_log_separation: f64,
}

/// Inequality constraint: crossover frequencies must be monotonically increasing
///
/// For parameters x = [gain_0, ..., gain_{N-1}, log10(xover_0), ..., log10(xover_{N-2})],
/// this constraint checks that each crossover frequency is strictly greater than the previous one.
///
/// Returns the maximum violation across all pairs. Feasible when <= 0.
///
/// # Arguments
/// * `x` - Parameter vector [gains, log10_crossover_freqs]
/// * `_grad` - Gradient (not computed)
/// * `data` - Constraint configuration
///
/// # Returns
/// Maximum constraint violation (negative = satisfied, positive = violated)
pub fn constraint_crossover_monotonicity(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut CrossoverMonotonicityConstraintData,
) -> f64 {
    let n_drivers = data.n_drivers;
    let n_xovers = n_drivers - 1;

    // Crossover frequencies start after the gains
    let xover_start = n_drivers;

    // Check all adjacent pairs of crossover frequencies
    let mut max_violation = f64::NEG_INFINITY;

    for i in 0..(n_xovers - 1) {
        let log_xover_i = x[xover_start + i];
        let log_xover_i_plus_1 = x[xover_start + i + 1];

        // Constraint: log_xover_i + min_separation < log_xover_i_plus_1
        // Rewritten: log_xover_i + min_separation - log_xover_i_plus_1 < 0
        let violation = log_xover_i + data.min_log_separation - log_xover_i_plus_1;

        max_violation = max_violation.max(violation);
    }

    max_violation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monotonic_crossovers() {
        let mut data = CrossoverMonotonicityConstraintData {
            n_drivers: 3,
            min_log_separation: 0.1,
        };

        // Test case: 3 drivers, 2 crossovers
        // gains: [0.0, 0.0, 0.0]
        // crossovers: [2.5, 3.0] (log10 space) => [316 Hz, 1000 Hz]
        let x = vec![0.0, 0.0, 0.0, 2.5, 3.0];

        let result = constraint_crossover_monotonicity(&x, None, &mut data);

        // Should be satisfied (negative or zero)
        assert!(
            result <= 0.0,
            "Monotonic crossovers should satisfy constraint"
        );
    }

    #[test]
    fn test_non_monotonic_crossovers() {
        let mut data = CrossoverMonotonicityConstraintData {
            n_drivers: 3,
            min_log_separation: 0.1,
        };

        // Test case: 3 drivers, 2 crossovers
        // crossovers: [3.0, 2.5] (log10 space) => [1000 Hz, 316 Hz] - WRONG ORDER!
        let x = vec![0.0, 0.0, 0.0, 3.0, 2.5];

        let result = constraint_crossover_monotonicity(&x, None, &mut data);

        // Should be violated (positive)
        assert!(
            result > 0.0,
            "Non-monotonic crossovers should violate constraint"
        );
    }

    #[test]
    fn test_too_close_crossovers() {
        let mut data = CrossoverMonotonicityConstraintData {
            n_drivers: 3,
            min_log_separation: 0.2, // Require at least 0.2 log10 separation
        };

        // Test case: crossovers too close together
        // crossovers: [2.5, 2.6] (log10 space) => [316 Hz, 398 Hz] - only 0.1 separation
        let x = vec![0.0, 0.0, 0.0, 2.5, 2.6];

        let result = constraint_crossover_monotonicity(&x, None, &mut data);

        // Should be violated (positive)
        assert!(
            result > 0.0,
            "Crossovers too close should violate constraint"
        );
    }

    #[test]
    fn test_four_driver_system() {
        let mut data = CrossoverMonotonicityConstraintData {
            n_drivers: 4,
            min_log_separation: 0.15,
        };

        // Test case: 4 drivers, 3 crossovers
        // crossovers: [2.5, 3.0, 3.5] (log10 space) => [316 Hz, 1000 Hz, 3162 Hz]
        let x = vec![0.0, 0.0, 0.0, 0.0, 2.5, 3.0, 3.5];

        let result = constraint_crossover_monotonicity(&x, None, &mut data);

        // Should be satisfied
        assert!(
            result <= 0.0,
            "Well-separated crossovers should satisfy constraint"
        );
    }
}
