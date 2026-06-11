// ============================================================================
// ESPRIT Frequency/Phase Estimator
// ============================================================================
//
// Super-resolution frequency estimation using the ESPRIT (Estimation of Signal
// Parameters via Rotational Invariance Techniques) algorithm.
//
// Resolves frequencies beyond FFT bin resolution by exploiting shift-invariance
// in the signal subspace of a Hankel data matrix.
//
// Complexity: O(M³) for SVD where M = signal.len() / 3 (typical).

use nalgebra::DMatrix;

/// A single estimated sinusoidal component.
#[derive(Debug, Clone)]
pub struct SinusoidEstimate {
    /// Frequency in Hz
    pub frequency: f64,
    /// Amplitude (linear scale)
    pub amplitude: f64,
    /// Phase in radians (-π to π)
    pub phase: f64,
}

/// Model order estimation criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOrderCriterion {
    /// Minimum Description Length — conservative, penalizes complexity more
    Mdl,
    /// Akaike Information Criterion — less conservative
    Aic,
}

/// Estimate the number of sinusoidal components using information-theoretic criteria.
///
/// # Arguments
/// * `singular_values` - Singular values from SVD of the Hankel matrix (descending order)
/// * `num_snapshots` - Number of rows (snapshots) in the Hankel matrix
/// * `criterion` - MDL or AIC
///
/// # Returns
/// Estimated model order (number of sinusoids)
pub fn estimate_model_order(
    singular_values: &[f64],
    num_snapshots: usize,
    criterion: ModelOrderCriterion,
) -> usize {
    let m = singular_values.len();
    let n = num_snapshots as f64;

    if m == 0 {
        return 0;
    }

    let mut best_p = 0;
    let mut best_cost = f64::INFINITY;

    // Eigenvalues = singular_values^2
    let eigenvalues: Vec<f64> = singular_values.iter().map(|s| s * s).collect();

    for p in 0..m {
        let noise_dim = m - p;
        if noise_dim == 0 {
            break;
        }

        // Geometric and arithmetic mean of noise eigenvalues
        let noise_eigs = &eigenvalues[p..];
        let arith_mean = noise_eigs.iter().sum::<f64>() / noise_dim as f64;

        if arith_mean <= 0.0 {
            break;
        }

        let log_geo_mean =
            noise_eigs.iter().map(|&e| (e.max(1e-30)).ln()).sum::<f64>() / noise_dim as f64;
        let geo_mean = log_geo_mean.exp();

        let ratio = geo_mean / arith_mean;
        if ratio <= 0.0 || !ratio.is_finite() {
            break;
        }

        let log_likelihood = -n * noise_dim as f64 * ratio.ln();

        let num_free_params = p as f64 * (2.0 * m as f64 - p as f64);

        let cost = match criterion {
            ModelOrderCriterion::Mdl => -log_likelihood + 0.5 * num_free_params * n.ln(),
            ModelOrderCriterion::Aic => -2.0 * log_likelihood + 2.0 * num_free_params,
        };

        if cost < best_cost {
            best_cost = cost;
            best_p = p;
        }
    }

    best_p
}

/// Core ESPRIT algorithm for super-resolution frequency estimation.
///
/// # Arguments
/// * `signal` - Input signal samples
/// * `sample_rate` - Sample rate in Hz
/// * `model_order` - Number of sinusoids to estimate (None = auto-detect via MDL)
/// * `window_size` - Hankel matrix column count (None = signal.len() / 3)
///
/// # Returns
/// Vector of estimated sinusoidal components, sorted by amplitude (descending)
pub fn esprit(
    signal: &[f32],
    sample_rate: f32,
    model_order: Option<usize>,
    window_size: Option<usize>,
) -> Vec<SinusoidEstimate> {
    let n = signal.len();
    if n < 4 {
        return Vec::new();
    }

    // Default M = N/3
    let m = window_size.unwrap_or(n / 3).max(2).min(n - 1);
    let num_rows = n - m + 1;

    if num_rows < 2 || m < 2 {
        return Vec::new();
    }

    // Build Hankel matrix X (num_rows x m)
    let hankel = DMatrix::from_fn(num_rows, m, |i, j| signal[i + j] as f64);

    // SVD
    let svd = hankel.svd(true, true);
    let singular_values = svd.singular_values.as_slice();

    // Guard against near-zero signals (degenerate subspace)
    if singular_values.is_empty() || singular_values[0] < f64::EPSILON * n as f64 {
        return Vec::new();
    }

    // Determine model order
    // For real-valued signals, each sinusoid contributes 2 eigenvalues (conjugate pair),
    // so we need to double the requested model order.
    let p = match model_order {
        Some(p) => (p * 2).min(m - 1).min(num_rows - 1),
        None => {
            let auto_p = estimate_model_order(singular_values, num_rows, ModelOrderCriterion::Mdl);
            // Ensure at least 2 for real signals
            auto_p.max(2).min(m - 1).min(num_rows - 1)
        }
    };

    if p == 0 {
        return Vec::new();
    }

    // Extract signal subspace: first P columns of V
    let v_full = match &svd.v_t {
        Some(v_t) => v_t.transpose(),
        None => return Vec::new(),
    };

    if v_full.ncols() < p || v_full.nrows() < m {
        return Vec::new();
    }

    let v_s = v_full.columns(0, p);

    // Shift-invariance: V_1 = V_s[0..m-1, :], V_2 = V_s[1..m, :]
    let v1 = v_s.rows(0, m - 1).clone_owned();
    let v2 = v_s.rows(1, m - 1).clone_owned();

    // Compute Phi = pinv(V_1) * V_2
    let v1_svd = v1.svd(true, true);
    let phi = match v1_svd.solve(&v2, 1e-10) {
        Ok(phi) => phi,
        Err(_) => return Vec::new(),
    };

    // Extract complex eigenvalues from real matrix
    // nalgebra::DMatrix<f64>::complex_eigenvalues() handles 2x2 blocks in real Schur form
    let eigenvalues = phi.complex_eigenvalues();

    let mut estimates = Vec::with_capacity(p);
    for lambda in eigenvalues.iter() {
        // Frequency from angle of eigenvalue
        let angle = lambda.im.atan2(lambda.re);
        let freq = sample_rate as f64 * angle / (2.0 * std::f64::consts::PI);

        // Only keep positive frequencies below Nyquist
        if freq > 0.0 && freq < sample_rate as f64 / 2.0 {
            let amplitude = estimate_amplitude(signal, freq, sample_rate as f64);
            let phase = estimate_phase(signal, freq, sample_rate as f64);

            estimates.push(SinusoidEstimate {
                frequency: freq,
                amplitude,
                phase,
            });
        }
    }

    // Sort by amplitude (descending)
    estimates.sort_by(|a, b| {
        b.amplitude
            .partial_cmp(&a.amplitude)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    estimates
}

/// Estimate the amplitude of a sinusoid at a given frequency using least-squares.
fn estimate_amplitude(signal: &[f32], freq: f64, sample_rate: f64) -> f64 {
    let n = signal.len();
    let omega = 2.0 * std::f64::consts::PI * freq / sample_rate;

    let mut sum_cos = 0.0;
    let mut sum_sin = 0.0;

    for (i, &s) in signal.iter().enumerate() {
        let phase = omega * i as f64;
        sum_cos += s as f64 * phase.cos();
        sum_sin += s as f64 * phase.sin();
    }

    2.0 * (sum_cos * sum_cos + sum_sin * sum_sin).sqrt() / n as f64
}

/// Estimate the phase of a sinusoid at a given frequency.
fn estimate_phase(signal: &[f32], freq: f64, sample_rate: f64) -> f64 {
    let omega = 2.0 * std::f64::consts::PI * freq / sample_rate;

    let mut sum_cos = 0.0;
    let mut sum_sin = 0.0;

    for (i, &s) in signal.iter().enumerate() {
        let phase = omega * i as f64;
        sum_cos += s as f64 * phase.cos();
        sum_sin += s as f64 * phase.sin();
    }

    sum_sin.atan2(sum_cos)
}

/// Convenience wrapper: estimate frequencies from a signal.
///
/// # Arguments
/// * `signal` - Input signal samples
/// * `sample_rate` - Sample rate in Hz
/// * `max_sinusoids` - Maximum number of sinusoids to return
///
/// # Returns
/// Sorted vector of estimated frequencies in Hz
pub fn estimate_frequencies(signal: &[f32], sample_rate: f32, max_sinusoids: usize) -> Vec<f64> {
    let estimates = esprit(signal, sample_rate, Some(max_sinusoids), None);
    let mut freqs: Vec<f64> = estimates
        .iter()
        .take(max_sinusoids)
        .map(|e| e.frequency)
        .collect();
    freqs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    freqs
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn gen_sinusoid(
        freq: f64,
        amplitude: f64,
        phase: f64,
        sample_rate: f64,
        num_samples: usize,
    ) -> Vec<f32> {
        (0..num_samples)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (amplitude * (2.0 * std::f64::consts::PI * freq * t + phase).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn test_pure_tone_1000_5_hz() {
        let sample_rate = 48000.0_f32;
        let freq = 1000.5;
        let signal = gen_sinusoid(freq, 1.0, 0.0, sample_rate as f64, 1024);

        let estimates = esprit(&signal, sample_rate, Some(1), None);
        assert!(
            !estimates.is_empty(),
            "ESPRIT should find at least one component"
        );

        let est_freq = estimates[0].frequency;
        let error = (est_freq - freq).abs();
        assert!(
            error < 1.0,
            "ESPRIT frequency error {error:.3} Hz exceeds 1 Hz threshold (estimated {est_freq:.3} vs actual {freq})"
        );
    }

    #[test]
    fn test_two_close_tones() {
        let sample_rate = 48000.0_f32;
        let signal1 = gen_sinusoid(1000.0, 1.0, 0.0, sample_rate as f64, 2048);
        let signal2 = gen_sinusoid(1050.0, 0.8, 0.5, sample_rate as f64, 2048);
        let signal: Vec<f32> = signal1.iter().zip(&signal2).map(|(&a, &b)| a + b).collect();

        let freqs = estimate_frequencies(&signal, sample_rate, 4);
        assert!(
            freqs.len() >= 2,
            "Should find at least 2 frequencies, found {}",
            freqs.len()
        );

        // Check that both frequencies are close to expected values
        let has_1000 = freqs.iter().any(|&f| (f - 1000.0).abs() < 5.0);
        let has_1050 = freqs.iter().any(|&f| (f - 1050.0).abs() < 5.0);
        assert!(has_1000, "Should find ~1000 Hz in {freqs:?}");
        assert!(has_1050, "Should find ~1050 Hz in {freqs:?}");
    }

    #[test]
    fn test_white_noise_low_model_order() {
        let _sample_rate = 48000.0_f32;
        // Generate pseudo-random noise using a simple LCG
        let mut rng_state: u64 = 12345;
        let signal: Vec<f32> = (0..512)
            .map(|_| {
                rng_state = rng_state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((rng_state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();

        let p = estimate_model_order(
            &{
                let hankel = DMatrix::from_fn(
                    signal.len() - signal.len() / 3 + 1,
                    signal.len() / 3,
                    |i, j| signal[i + j] as f64,
                );
                let svd = hankel.svd(false, false);
                svd.singular_values.as_slice().to_vec()
            },
            signal.len() - signal.len() / 3 + 1,
            ModelOrderCriterion::Mdl,
        );

        assert!(p <= 5, "White noise model order should be small, got {p}");
    }

    #[test]
    fn test_three_sinusoids_known_answer() {
        let sample_rate = 48000.0_f32;
        let freqs_expected = [440.0, 880.0, 1320.0];
        let amps = [1.0, 0.5, 0.3];
        let phases = [0.0, 0.7, -1.2];

        let num_samples = 4096;
        let mut signal = vec![0.0f32; num_samples];
        for ((&freq, &amp), &phase) in freqs_expected.iter().zip(&amps).zip(&phases) {
            let s = gen_sinusoid(freq, amp, phase, sample_rate as f64, num_samples);
            for (i, &v) in s.iter().enumerate() {
                signal[i] += v;
            }
        }

        let estimates = esprit(&signal, sample_rate, Some(3), None);
        assert!(
            estimates.len() >= 3,
            "Should find 3 sinusoids, found {}",
            estimates.len()
        );

        let est_freqs = estimate_frequencies(&signal, sample_rate, 3);
        for &expected in &freqs_expected {
            assert!(
                est_freqs.iter().any(|&f| (f - expected).abs() < 3.0),
                "Expected frequency {expected} Hz not found in {est_freqs:?}"
            );
        }
    }

    #[test]
    fn test_near_zero_signal_no_phantom() {
        // Regression: near-zero signals should not produce phantom frequencies
        let signal = vec![1e-38f32; 512];
        let result = esprit(&signal, 48000.0, Some(2), None);
        assert!(
            result.is_empty(),
            "Near-zero signal should produce no estimates, got {} components",
            result.len()
        );
    }

    #[test]
    fn test_all_zero_signal() {
        let signal = vec![0.0f32; 512];
        let result = esprit(&signal, 48000.0, Some(2), None);
        assert!(
            result.is_empty(),
            "All-zero signal should produce no estimates"
        );
    }

    #[test]
    fn test_empty_signal() {
        let result = esprit(&[], 48000.0, Some(1), None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_very_short_signal() {
        let result = esprit(&[1.0, 2.0], 48000.0, Some(1), None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_estimate_frequencies_convenience() {
        let sample_rate = 48000.0_f32;
        let signal = gen_sinusoid(500.0, 1.0, 0.0, sample_rate as f64, 2048);
        let freqs = estimate_frequencies(&signal, sample_rate, 2);
        assert!(!freqs.is_empty());
        assert!(
            (freqs[0] - 500.0).abs() < 2.0,
            "Expected ~500 Hz, got {:.2}",
            freqs[0]
        );
    }

    #[test]
    fn test_esprit_exactly_four_samples() {
        let signal = vec![1.0_f32, 0.0, -1.0, 0.0];
        let result = esprit(&signal, 48000.0, Some(1), None);
        assert!(
            result.len() <= 1,
            "4-sample signal should return at most 1 estimate"
        );
    }

    #[test]
    fn test_esprit_dc_component_rejected() {
        let sample_rate = 48000.0_f32;
        let signal = vec![0.1_f32; 1024];
        let estimates = esprit(&signal, sample_rate, Some(1), None);
        for est in &estimates {
            assert!(
                est.frequency > 1.0,
                "DC component should be rejected, got {} Hz",
                est.frequency
            );
        }
    }

    #[test]
    fn test_esprit_amplitude_accuracy() {
        let sample_rate = 48000.0_f32;
        let freq = 1000.0;
        let amp = 0.75;
        let signal = gen_sinusoid(freq, amp, 0.0, sample_rate as f64, 2048);
        let estimates = esprit(&signal, sample_rate, Some(1), None);
        assert!(!estimates.is_empty());
        let est_amp = estimates[0].amplitude;
        assert!(
            (est_amp - amp).abs() < 0.05,
            "Amplitude error too large: estimated {est_amp}, expected {amp}"
        );
    }

    #[test]
    fn test_esprit_auto_model_order_on_noise() {
        let mut rng_state: u64 = 12345;
        let signal: Vec<f32> = (0..512)
            .map(|_| {
                rng_state = rng_state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((rng_state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();
        let estimates = esprit(&signal, 48000.0, None, None);
        assert!(
            estimates.len() <= 3,
            "Noise should produce <= 3 estimates, got {}",
            estimates.len()
        );
    }

    #[test]
    fn test_esprit_nyquist_boundary() {
        let sample_rate = 48000.0_f32;
        let freq = sample_rate as f64 / 2.0 - 100.0;
        let signal = gen_sinusoid(freq, 1.0, 0.0, sample_rate as f64, 2048);
        let estimates = esprit(&signal, sample_rate, Some(1), None);
        assert!(!estimates.is_empty());
        assert!(
            (estimates[0].frequency - freq).abs() < 5.0,
            "Near-Nyquist frequency error too large"
        );
    }
}
