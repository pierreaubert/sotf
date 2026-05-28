// ============================================================================
// FFT Operations for Denoiser
// ============================================================================

use super::DenoiserPlugin;
impl DenoiserPlugin {
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
                self.freq_domain[ch][k] *= gain;
            }

            // Inverse FFT (Complex -> Real)
            self.fft_inverse
                .process(&mut self.freq_domain[ch], &mut self.time_out_channels[ch])
                .expect("FFT inverse failed");
        }
    }

    /// Get power spectrum for a specific channel (avoids allocation)
    #[inline]
    pub(super) fn get_power_at_bin(&self, channel: usize, bin: usize) -> f32 {
        self.freq_domain[channel][bin].norm_sqr()
    }
}
