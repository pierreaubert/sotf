// ============================================================================
// FFT Operations for Denoiser
// ============================================================================

use super::DenoiserPlugin;
impl DenoiserPlugin {
    /// Apply window and forward FFT for all channels
    /// Input: interleaved audio [L0, R0, L1, R1, ...]
    /// Output: freq_domain buffers are filled with complex spectrum
    pub(super) fn apply_window_and_forward_fft(&mut self, input: &[f32]) -> Result<(), String> {
        // Optimization: De-interleave and window in a cache-friendly order
        // We iterate time (i) then channels (ch) to read 'input' linearly
        for i in 0..self.config.fft_size {
            let window_val = self.fft.window[i];
            for ch in 0..self.config.channels {
                let idx = i * self.config.channels + ch;
                self.fft.time_domain[ch][i] = input[idx] * window_val;
            }
        }

        // Perform Forward FFT (Real -> Complex) for all channels
        for ch in 0..self.config.channels {
            self.fft
                .fft_forward
                .process(&mut self.fft.time_domain[ch], &mut self.fft.freq_domain[ch])
                .map_err(|e| format!("FFT forward failed: {:?}", e))?;
        }
        Ok(())
    }

    /// Apply Wiener gains and perform inverse FFT for all channels
    /// Output: time_out_channels buffers are filled with processed samples
    pub(super) fn apply_gains_and_inverse_fft(&mut self) -> Result<(), String> {
        for ch in 0..self.config.channels {
            // Apply smoothed Wiener gains to frequency domain
            for k in 0..self.config.spectrum_size {
                let gain = self.gains.smoothed_gain[ch][k];
                self.fft.freq_domain[ch][k] *= gain;
            }

            // Inverse FFT (Complex -> Real)
            self.fft
                .fft_inverse
                .process(
                    &mut self.fft.freq_domain[ch],
                    &mut self.io.time_out_channels[ch],
                )
                .map_err(|e| format!("FFT inverse failed: {:?}", e))?;
        }
        Ok(())
    }

    /// Get power spectrum for a specific channel (avoids allocation)
    #[inline]
    pub(super) fn get_power_at_bin(&self, channel: usize, bin: usize) -> f32 {
        self.fft.freq_domain[channel][bin].norm_sqr()
    }
}
