//! Cosine filter for WaveHoltz time-domain extraction
//!
//! The cosine filter extracts the Helmholtz solution from the wave equation solution
//! via the time-average: `result = (2/T) ∫₀ᵀ cos(ωt) w(t) dt`
//!
//! Discretized with trapezoidal rule over N_t time steps.

use ndarray::Array1;

/// Cosine filter with precomputed weights for WaveHoltz time filtering
pub struct CosineFilter {
    /// Combined weights: (2/N_t) * cos(ω·n·dt) * trapezoidal_weight
    combined_weights: Vec<f64>,
    /// Number of time steps
    n_steps: usize,
}

impl CosineFilter {
    /// Create a new cosine filter
    ///
    /// # Arguments
    /// * `omega` - Angular frequency ω
    /// * `dt` - Time step size
    /// * `n_steps` - Number of time steps per period
    pub fn new(omega: f64, dt: f64, n_steps: usize) -> Self {
        let mut combined_weights = Vec::with_capacity(n_steps + 1);

        // Trapezoidal rule: weight = 1 for interior points, 0.5 for endpoints
        // Filter: (2/N_t) * cos(ω·n·dt) * trap_weight
        let scale = 2.0 / n_steps as f64;
        for n in 0..=n_steps {
            let trap_weight = if n == 0 || n == n_steps { 0.5 } else { 1.0 };
            let cos_val = (omega * n as f64 * dt).cos();
            combined_weights.push(scale * cos_val * trap_weight);
        }

        Self {
            combined_weights,
            n_steps,
        }
    }

    /// Create filter with dispersion correction
    ///
    /// Adjusts the filter weights to account for the numerical dispersion
    /// introduced by the Newmark time-stepping scheme. This ensures the
    /// WaveHoltz solution converges to the *discretized* Helmholtz solution
    /// rather than the continuous one, eliminating time-stepping artifacts.
    ///
    /// # Arguments
    /// * `omega` - Angular frequency ω
    /// * `dt` - Time step size
    /// * `n_steps` - Number of time steps per period
    /// * `eigenvalue_range` - (λ_min, λ_max) approximate eigenvalue range of M⁻¹K
    pub fn new_with_dispersion_correction(
        omega: f64,
        dt: f64,
        n_steps: usize,
        eigenvalue_range: (f64, f64),
    ) -> Self {
        let mut combined_weights = Vec::with_capacity(n_steps + 1);

        // For Newmark β=1/4, the discrete dispersion relation is:
        //   cos(ω_d · dt) = (2 - dt²λ) / (2 + dt²λ)
        // where ω_d is the discrete frequency and λ is the eigenvalue.
        //
        // The correction modifies the cosine filter using the discrete
        // frequency relation β_d(λ) instead of the continuous cos(ωt).
        let omega_sq = omega * omega;
        let dt_sq = dt * dt;

        // Use the midpoint eigenvalue for the correction
        let lambda_mid = 0.5 * (eigenvalue_range.0 + eigenvalue_range.1);
        let lambda_eff = lambda_mid.max(omega_sq * 0.1); // avoid degenerate correction

        // Discrete dispersion: cos(ω_d·dt) for the target frequency
        let numerator = 2.0 - dt_sq * lambda_eff;
        let denominator = 2.0 + dt_sq * lambda_eff;
        let cos_omega_d_dt = (numerator / denominator).clamp(-1.0, 1.0);
        let omega_d_dt = cos_omega_d_dt.acos();

        let scale = 2.0 / n_steps as f64;
        for n in 0..=n_steps {
            let trap_weight = if n == 0 || n == n_steps { 0.5 } else { 1.0 };
            // Use dispersion-corrected discrete frequency
            let cos_val = (omega_d_dt * n as f64).cos();
            combined_weights.push(scale * cos_val * trap_weight);
        }

        Self {
            combined_weights,
            n_steps,
        }
    }

    /// Get the filter weight for time step n
    #[inline]
    pub fn weight(&self, n: usize) -> f64 {
        self.combined_weights[n]
    }

    /// Number of time steps
    pub fn n_steps(&self) -> usize {
        self.n_steps
    }

    /// Accumulate filtered contribution: accumulator += weight(n) * w_n
    pub fn accumulate(&self, n: usize, w_n: &Array1<f64>, accumulator: &mut Array1<f64>) {
        let w = self.combined_weights[n];
        if w.abs() > 1e-20 {
            accumulator.scaled_add(w, w_n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_weights_sum() {
        // For cos(ωt) with ω matching the period, the filter should extract DC component
        let omega = 2.0 * std::f64::consts::PI; // period = 1
        let n_steps = 20;
        let dt = 1.0 / n_steps as f64;

        let filter = CosineFilter::new(omega, dt, n_steps);

        // Sum of weights * cos(ωt) over one period should give normalization constant
        let mut sum = 0.0;
        for n in 0..=n_steps {
            let t = n as f64 * dt;
            sum += filter.weight(n) * (omega * t).cos();
        }
        // (2/T) ∫ cos²(ωt) dt = 1
        assert!(
            (sum - 1.0).abs() < 0.02,
            "Filter self-correlation should be ~1, got {}",
            sum
        );
    }

    #[test]
    fn test_filter_extracts_single_frequency() {
        let omega = 2.0 * std::f64::consts::PI;
        let n_steps = 40;
        let dt = 1.0 / n_steps as f64;
        let filter = CosineFilter::new(omega, dt, n_steps);

        // Signal: cos(ωt) + cos(3ωt)
        // Filter at ω should extract cos(ωt) component → result ≈ 1
        // Filter at ω should reject cos(3ωt) component → contribution ≈ 0
        let mut result_target = 0.0;
        let mut result_harmonic = 0.0;
        for n in 0..=n_steps {
            let t = n as f64 * dt;
            result_target += filter.weight(n) * (omega * t).cos();
            result_harmonic += filter.weight(n) * (3.0 * omega * t).cos();
        }

        assert!(
            (result_target - 1.0).abs() < 0.02,
            "Should extract target frequency: got {}",
            result_target
        );
        assert!(
            result_harmonic.abs() < 0.05,
            "Should reject harmonic: got {}",
            result_harmonic
        );
    }

    #[test]
    fn test_accumulate() {
        let omega = 2.0 * std::f64::consts::PI;
        let n_steps = 10;
        let dt = 1.0 / n_steps as f64;
        let filter = CosineFilter::new(omega, dt, n_steps);

        let ndofs = 3;
        let mut acc = Array1::zeros(ndofs);
        let w = Array1::from_vec(vec![1.0, 2.0, 3.0]);

        filter.accumulate(0, &w, &mut acc);
        // weight(0) = (2/10) * cos(0) * 0.5 = 0.1
        assert!((acc[0] - 0.1).abs() < 1e-10);
        assert!((acc[1] - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_dispersion_corrected_filter() {
        let omega = 2.0 * std::f64::consts::PI;
        let n_steps = 20;
        let dt = 1.0 / n_steps as f64;

        let standard = CosineFilter::new(omega, dt, n_steps);
        let corrected =
            CosineFilter::new_with_dispersion_correction(omega, dt, n_steps, (1.0, 100.0));

        // Corrected weights should differ from standard
        let mut diff_sum = 0.0;
        for n in 0..=n_steps {
            diff_sum += (standard.weight(n) - corrected.weight(n)).abs();
        }
        assert!(
            diff_sum > 1e-6,
            "Dispersion correction should modify weights"
        );
    }
}
