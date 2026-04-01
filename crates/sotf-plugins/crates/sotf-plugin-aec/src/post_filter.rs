// ============================================================================
// Residual Echo Suppression — Wiener-style Post-Filter
// ============================================================================
//
// Suppresses residual echo that the adaptive filter couldn't fully cancel.
// Uses a spectral gain approach: estimate the echo power, compute a Wiener-like
// gain, and apply it to the error signal.

use rustfft::num_complex::Complex;

/// Residual echo suppression post-filter.
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
            output_buf: vec![Complex::new(0.0, 0.0); spectrum_size * 2],
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
        if self.output_buf.len() < n {
            self.output_buf.resize(n, Complex::new(0.0, 0.0));
        }

        for k in 0..n {
            if k < self.spectrum_size {
                let s_error = error_spectrum[k].norm_sqr();
                let s_echo = echo_estimate_spectrum[k].norm_sqr();

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

                self.output_buf[k] = error_spectrum[k] * self.smoothed_gains[k];
            } else {
                self.output_buf[k] = error_spectrum[k];
            }
        }

        &self.output_buf[..n]
    }

    /// Reset suppressor state.
    pub fn reset(&mut self) {
        self.smoothed_gains.fill(1.0);
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
