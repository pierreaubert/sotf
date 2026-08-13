// ============================================================================
// Residual Echo Suppression — Wiener-style Post-Filter
// ============================================================================
//
// Suppresses residual echo that the adaptive filter couldn't fully cancel.
// Uses a spectral gain approach: estimate the echo power, compute a Wiener-like
// gain, and apply it to the error signal.

use rustfft::num_complex::Complex;

/// Residual echo suppression post-filter.
///
/// Includes a simple power-ratio double-talk detector (DTD): when the
/// microphone power significantly exceeds the echo estimate power, near-end
/// speech is likely present and the suppressor is bypassed to avoid muffling
/// it.
#[derive(Debug)]
pub struct ResidualEchoSuppressor {
    /// Over-subtraction factor (1.0-2.0, higher = more aggressive)
    beta: f32,
    /// Spectral floor (minimum gain to prevent artifacts)
    g_min: f32,
    /// Smoothed gains per bin
    smoothed_gains: Vec<f32>,
    /// Gain smoothing factor
    gain_attack_alpha: f32,
    gain_release_alpha: f32,
    spectrum_size: usize,
    /// Pre-allocated output buffer (avoids hot-path allocation)
    output_buf: Vec<Complex<f32>>,
    // ---- Double-talk detector state ----
    /// Smoothed microphone power (across all bins)
    dtd_mic_power: f32,
    /// Smoothed echo-estimate power (across all bins)
    dtd_echo_power: f32,
    /// Power smoothing factor for DTD (slower than per-bin gain smoothing)
    dtd_alpha: f32,
    /// DTD threshold: when mic_power > dtd_threshold * echo_power → double-talk
    /// 6 dB = power ratio 4.0 gives adequate margin.
    dtd_threshold: f32,
    /// Smoothed fraction of the cancelled echo expected to remain in error.
    residual_leakage: f32,
    leakage_alpha: f32,
}

impl ResidualEchoSuppressor {
    /// Create a new residual echo suppressor.
    ///
    /// # Arguments
    /// * `spectrum_size` - Number of frequency bins (fft_size / 2 + 1 or fft_size for complex FFT)
    /// * `beta` - Over-subtraction factor (default 1.5)
    /// * `g_min` - Spectral floor in linear (default 0.056 ≈ -25 dB)
    #[cfg(test)]
    pub fn new(spectrum_size: usize, beta: f32, g_min: f32) -> Self {
        Self::new_with_timing(spectrum_size, beta, g_min, 256, 48_000)
    }

    pub fn new_with_timing(
        spectrum_size: usize,
        beta: f32,
        g_min: f32,
        block_size: usize,
        sample_rate: u32,
    ) -> Self {
        let block_seconds = block_size as f32 / sample_rate.max(1) as f32;
        let coefficient = |seconds: f32| (-block_seconds / seconds).exp();
        Self {
            beta,
            g_min,
            smoothed_gains: vec![1.0; spectrum_size],
            gain_attack_alpha: coefficient(0.010),
            gain_release_alpha: coefficient(0.080),
            spectrum_size,
            output_buf: vec![Complex::new(0.0, 0.0); spectrum_size],
            dtd_mic_power: 0.0,
            dtd_echo_power: 0.0,
            dtd_alpha: coefficient(0.050),
            dtd_threshold: 1.2,
            residual_leakage: 0.1,
            leakage_alpha: coefficient(0.250),
        }
    }

    /// Compute and apply suppression gains.
    ///
    /// # Arguments
    /// * `error_spectrum` - FFT of the error (AEC output) signal
    /// * `echo_estimate_spectrum` - FFT of the echo estimate from the adaptive filter
    ///
    /// # Returns
    /// Suppressed spectrum (apply IFFT externally)
    pub fn process(
        &mut self,
        error_spectrum: &[Complex<f32>],
        echo_estimate_spectrum: &[Complex<f32>],
    ) -> &[Complex<f32>] {
        debug_assert!(error_spectrum.len() >= self.spectrum_size);
        debug_assert!(echo_estimate_spectrum.len() >= self.spectrum_size);

        let n = error_spectrum.len();
        debug_assert_eq!(
            self.output_buf.len(),
            n,
            "output_buf size should equal spectrum size"
        );

        // Double-talk detector: compare total mic power to total echo-estimate
        // power.  If the microphone (error) energy substantially exceeds the
        // echo estimate energy, a near-end speaker is active and we must not
        // over-suppress.
        let raw_mic_pwr: f32 = error_spectrum[..self.spectrum_size]
            .iter()
            .map(|c| c.norm_sqr())
            .sum::<f32>();
        let raw_echo_pwr: f32 = echo_estimate_spectrum[..self.spectrum_size]
            .iter()
            .map(|c| c.norm_sqr())
            .sum::<f32>();
        self.dtd_mic_power =
            self.dtd_alpha * self.dtd_mic_power + (1.0 - self.dtd_alpha) * raw_mic_pwr;
        self.dtd_echo_power =
            self.dtd_alpha * self.dtd_echo_power + (1.0 - self.dtd_alpha) * raw_echo_pwr;

        // Once an echo estimate exists, comparable residual/error power is
        // evidence of near-end speech.  Unlike the former 6 dB rule this also
        // protects balanced double-talk.
        // In that case bypass suppression (copy input directly).
        let echo_active = self.dtd_echo_power > 1e-12;
        let double_talk = echo_active
            && (raw_mic_pwr > self.dtd_threshold * (raw_echo_pwr + 1e-20)
                || self.dtd_mic_power > self.dtd_threshold * self.dtd_echo_power);

        if echo_active && !double_talk {
            let observed = (raw_mic_pwr / (raw_echo_pwr + 1e-20)).clamp(0.001, 1.0);
            self.residual_leakage =
                self.leakage_alpha * self.residual_leakage + (1.0 - self.leakage_alpha) * observed;
        }

        // n == spectrum_size (invariant enforced by debug_assert_eq! above),
        // so the inner `k < self.spectrum_size` branch is always taken.
        if double_talk {
            // Pass through without modification — reset gains toward 1.0 so
            // there is no abrupt change when double-talk ends.
            for (k, (out, &e)) in self
                .output_buf
                .iter_mut()
                .zip(error_spectrum.iter())
                .enumerate()
            {
                self.smoothed_gains[k] = self.gain_release_alpha * self.smoothed_gains[k]
                    + (1.0 - self.gain_release_alpha);
                *out = e;
            }
        } else {
            for (k, (out, (&e, &echo))) in self
                .output_buf
                .iter_mut()
                .zip(error_spectrum.iter().zip(echo_estimate_spectrum.iter()))
                .enumerate()
            {
                let s_error = e.norm_sqr();
                let s_echo = echo.norm_sqr() * self.residual_leakage;

                // Wiener-style gain: G = max((|E|² - β|Ê|²) / |E|², G_min)
                let speech_est = (s_error - self.beta * s_echo).max(0.0);
                let gain = if s_error > 1e-20 {
                    (speech_est / s_error).max(self.g_min)
                } else {
                    self.g_min
                };

                // Smooth gains temporally
                let alpha = if gain < self.smoothed_gains[k] {
                    self.gain_attack_alpha
                } else if raw_mic_pwr > 2.0 * self.residual_leakage * (raw_echo_pwr + 1e-20) {
                    // Near-end onset: release suppression immediately rather
                    // than smearing the first phonemes over the release tail.
                    0.0
                } else {
                    self.gain_release_alpha
                };
                self.smoothed_gains[k] = alpha * self.smoothed_gains[k] + (1.0 - alpha) * gain;

                *out = e * self.smoothed_gains[k];
            }
        }

        &self.output_buf[..n]
    }

    /// Reset suppressor state.
    pub fn reset(&mut self) {
        self.smoothed_gains.fill(1.0);
        self.dtd_mic_power = 0.0;
        self.dtd_echo_power = 0.0;
        self.residual_leakage = 0.1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suppressor_creation() {
        let sup = ResidualEchoSuppressor::new(257, 1.5, 0.056);
        assert_eq!(sup.spectrum_size, 257);
    }

    #[test]
    fn test_no_echo_passthrough() {
        let mut sup = ResidualEchoSuppressor::new(8, 1.5, 0.01);

        // Error with no echo estimate → should pass through mostly unchanged
        let error: Vec<Complex<f32>> = (0..8).map(|i| Complex::new(i as f32 * 0.1, 0.0)).collect();
        let echo_est = vec![Complex::new(0.0, 0.0); 8];

        let output = sup.process(&error, &echo_est);
        assert_eq!(output.len(), 8);

        // With zero echo estimate, gain should approach 1.0
        // (smoothed from initial 1.0, new gain is also 1.0)
        for (i, c) in output.iter().enumerate() {
            assert!(
                (c.re - error[i].re).abs() < 0.05,
                "Bin {i}: expected ~{}, got {}",
                error[i].re,
                c.re
            );
        }
    }

    #[test]
    fn test_full_echo_suppression() {
        let mut sup = ResidualEchoSuppressor::new(8, 1.5, 0.01);

        // Echo estimate equals error → should suppress to floor
        let spectrum: Vec<Complex<f32>> = (0..8).map(|_| Complex::new(1.0, 0.0)).collect();

        // Process multiple frames to let smoothing converge
        for _ in 0..20 {
            let _ = sup.process(&spectrum, &spectrum);
        }
        let output = sup.process(&spectrum, &spectrum);

        // Each bin should be attenuated significantly
        for (i, c) in output.iter().enumerate() {
            assert!(
                c.norm() < 0.5,
                "Bin {i} should be suppressed, got magnitude {}",
                c.norm()
            );
        }
    }

    #[test]
    fn test_reset() {
        let mut sup = ResidualEchoSuppressor::new(8, 1.5, 0.056);
        let spectrum = vec![Complex::new(1.0, 0.0); 8];
        let _ = sup.process(&spectrum, &spectrum);

        sup.reset();
        for &g in &sup.smoothed_gains {
            assert_eq!(g, 1.0);
        }
    }

    #[test]
    fn leakage_model_preserves_near_end_during_balanced_double_talk() {
        let mut sup = ResidualEchoSuppressor::new_with_timing(33, 1.5, 0.01, 64, 48_000);
        let echo = vec![Complex::new(0.4, 0.0); 33];
        let far_only_residual = vec![Complex::new(0.04, 0.0); 33];
        for _ in 0..50 {
            let _ = sup.process(&far_only_residual, &echo);
        }
        for near_to_far in [0.25_f32, 1.0, 2.0] {
            let near = 0.4 * near_to_far.sqrt();
            let error = vec![Complex::new(near + 0.04, 0.0); 33];
            let output = sup.process(&error, &echo);
            let near_in_power = near * near * 33.0;
            let out_power: f32 = output.iter().map(|x| x.norm_sqr()).sum();
            let loss_db = 10.0 * (near_in_power / out_power.max(1e-20)).log10();
            assert!(
                loss_db < 3.0,
                "near/far={near_to_far} near-end loss was {loss_db} dB"
            );
        }
    }

    #[test]
    fn smoothing_coefficients_follow_seconds_not_blocks() {
        let low = ResidualEchoSuppressor::new_with_timing(33, 1.5, 0.01, 64, 48_000);
        let high = ResidualEchoSuppressor::new_with_timing(33, 1.5, 0.01, 64, 96_000);
        assert!(high.dtd_alpha > low.dtd_alpha);
        assert!(high.gain_release_alpha > low.gain_release_alpha);
    }
}
