// ============================================================================
// FFT Operations for Denoiser
// ============================================================================

use super::{DenoiserConfig, DenoiserFft, DenoiserPlugin};

/// Apply the analysis window and forward FFT using explicitly disjoint state.
/// Keeping this helper outside `DenoiserPlugin` avoids aliasing a slice inside
/// the plugin while also borrowing the whole plugin mutably.
pub(super) fn apply_window_and_forward_fft(
    config: &DenoiserConfig,
    fft: &mut DenoiserFft,
    input: &[f32],
) -> Result<(), String> {
    for i in 0..config.fft_size {
        let window_val = fft.window[i];
        for ch in 0..config.channels {
            let idx = i * config.channels + ch;
            fft.time_domain[ch][i] = input[idx] * window_val;
        }
    }

    for ch in 0..config.channels {
        fft.fft_forward
            .process(&mut fft.time_domain[ch], &mut fft.freq_domain[ch])
            .map_err(|e| format!("FFT forward failed: {e:?}"))?;
    }
    Ok(())
}

impl DenoiserPlugin {
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
