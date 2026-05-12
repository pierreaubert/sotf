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
    gain_alpha: f32,
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
}

impl ResidualEchoSuppressor {
    /// Create a new residual echo suppressor.
    ///
    /// # Arguments
    /// * `spectrum_size` - Number of frequency bins (fft_size / 2 + 1 or fft_size for complex FFT)
    /// * `beta` - Over-subtraction factor (default 1.5)
    /// * `g_min` - Spectral floor in linear (default 0.056 ≈ -25 dB)
    pub fn new(spectrum_size: usize, beta: f32, g_min: f32) -> Self {
        Self {
            beta,
            g_min,
            smoothed_gains: vec![1.0; spectrum_size],
            gain_alpha: 0.8,
            spectrum_size,
            output_buf: vec![Complex::new(0.0, 0.0); spectrum_size],
            dtd_mic_power: 0.0,
            dtd_echo_power: 0.0,
            dtd_alpha: 0.9,
            dtd_threshold: 4.0, // 6 dB advantage → near-end speech detected
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

        // Double-talk detected when mic power >> echo power by dtd_threshold.
        // In that case bypass suppression (copy input directly).
        let double_talk = self.dtd_mic_power > self.dtd_threshold * (self.dtd_echo_power + 1e-20);

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
                self.smoothed_gains[k] =
                    self.gain_alpha * self.smoothed_gains[k] + (1.0 - self.gain_alpha) * 1.0;
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
                let s_echo = echo.norm_sqr();

                // Wiener-style gain: G = max((|E|² - β|Ê|²) / |E|², G_min)
                let speech_est = (s_error - self.beta * s_echo).max(0.0);
                let gain = if s_error > 1e-20 {
                    (speech_est / s_error).max(self.g_min)
                } else {
                    self.g_min
                };

                // Smooth gains temporally
                self.smoothed_gains[k] =
                    self.gain_alpha * self.smoothed_gains[k] + (1.0 - self.gain_alpha) * gain;

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
    }

    /// Set over-subtraction factor.
    pub fn set_beta(&mut self, beta: f32) {
        self.beta = beta.clamp(0.5, 4.0);
    }

    /// Set spectral floor.
    pub fn set_g_min(&mut self, g_min: f32) {
        self.g_min = g_min.clamp(0.001, 0.5);
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
}
