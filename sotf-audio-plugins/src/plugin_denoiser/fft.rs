// ============================================================================
// FFT Operations for Denoiser
// ============================================================================

use super::DenoiserPlugin;
use rustfft::num_complex::Complex;

impl DenoiserPlugin {
    /// Generate Hann window for STFT analysis
    /// Uses N (not N-1) divisor for perfect COLA with 50% overlap
    pub(super) fn generate_hann_window(fft_size: usize) -> Vec<f32> {
        (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / fft_size as f32).cos())
            })
            .collect()
    }

    /// Apply window and forward FFT for all channels
    /// Input: interleaved audio [L0, R0, L1, R1, ...]
    /// Output: freq_domain buffers are filled with complex spectrum
    pub(super) fn apply_window_and_forward_fft(&mut self, input: &[f32]) {
        // Optimization: De-interleave and window in a cache-friendly order
        // We iterate time (i) then channels (ch) to read 'input' linearly
        for i in 0..self.fft_size {
            let window_val = self.window[i];
            for ch in 0..self.channels {
                let idx = i * self.channels + ch;
                self.time_domain[ch][i] = input[idx] * window_val;
            }
        }

        // Perform Forward FFT (Real -> Complex) for all channels
        for ch in 0..self.channels {
            self.fft_forward
                .process(&mut self.time_domain[ch], &mut self.freq_domain[ch])
                .expect("FFT forward failed");
        }
    }

    /// Apply Wiener gains and perform inverse FFT for all channels
    /// Output: time_out_channels buffers are filled with processed samples
    pub(super) fn apply_gains_and_inverse_fft(&mut self) {
        for ch in 0..self.channels {
            // Apply smoothed Wiener gains to frequency domain
            for k in 0..self.spectrum_size {
                let gain = self.smoothed_gain[ch][k];
                self.freq_domain[ch][k] = self.freq_domain[ch][k] * gain;
            }

            // Inverse FFT (Complex -> Real)
            self.fft_inverse
                .process(&mut self.freq_domain[ch], &mut self.time_out_channels[ch])
                .expect("FFT inverse failed");
        }
    }

    /// Calculate power spectrum for a channel
    /// Returns |X(k)|^2 for each frequency bin
    #[inline]
    #[allow(dead_code)]
    pub(super) fn calculate_power_spectrum(freq: &[Complex<f32>]) -> Vec<f32> {
        freq.iter().map(|c| c.norm_sqr()).collect()
    }

    /// Get power spectrum for a specific channel (avoids allocation)
    #[inline]
    pub(super) fn get_power_at_bin(&self, channel: usize, bin: usize) -> f32 {
        self.freq_domain[channel][bin].norm_sqr()
    }
}
