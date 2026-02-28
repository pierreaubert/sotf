// ============================================================================
// Polyphonic Note Detection Gain Calculation
// ============================================================================
//
// This module implements a "Spectral Gate" or "Spectral Subtraction" approach
// focused on preserving tonal content (notes) while heavily attenuating background.
//
// It uses the noise estimate from MCRA to determine the Signal-to-Noise Ratio (SNR)
// per bin. If the SNR exceeds a threshold, the bin is considered "signal" (a note)
// and passed with unity gain. Otherwise, it is attenuated to the floor level.

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

impl DenoiserPlugin {
    /// Calculate and apply Polyphonic Note Detection (Spectral Gate) gains
    ///
    /// Three-pass approach:
    /// 1. Compute binary gate gains per bin based on SNR threshold
    /// 2. Smooth gains across frequency bins (prevents musical noise)
    /// 3. Apply temporal smoothing with attack/release envelope
    pub(super) fn calculate_polyphonic_gains(&mut self) {
        let floor_linear = self.floor_linear;
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;

        let mut total_reduction = 0.0_f32;
        let mut bin_count = 0;

        for ch in 0..self.channels {
            // Pass 1: Compute instantaneous gate gain based on PND peaks
            for k in 0..self.spectrum_size {
                self.gain[ch][k] = floor_linear;
            }

            // Apply unity gain around detected tonal peaks
            let peaks = self.pnd_analyzers[ch].current_matched_peaks();
            for &(freq_hz, _mag) in peaks {
                let center_k = (freq_hz / bin_hz).round() as usize;

                // Spread unity gain to adjacent bins to account for spectral leakage
                let start_k = center_k.saturating_sub(1);
                let end_k = (center_k + 1).min(self.spectrum_size - 1);

                for k in start_k..=end_k {
                    self.gain[ch][k] = 1.0;
                }
            }

            // Pass 2: Smooth gains across frequency bins
            self.smooth_gains_across_frequency(ch);

            // Pass 2b: Psychoacoustic masking — skip denoising for masked bins
            if self.psychoacoustic_masking {
                self.compute_masking_thresholds(ch);
                for k in 0..self.spectrum_size {
                    if self.is_noise_masked(ch, k) {
                        self.gain[ch][k] = 1.0;
                    }
                }
            }

            // Pass 3: Apply temporal smoothing with attack/release
            for k in 0..self.spectrum_size {
                let target_gain = self.gain[ch][k];
                let prev_gain = self.smoothed_gain[ch][k];
                let coeff = if target_gain > prev_gain {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };
                let smoothed = target_gain + coeff * (prev_gain - target_gain);
                self.smoothed_gain[ch][k] = smoothed;

                total_reduction += (1.0 - smoothed).max(0.0);
                bin_count += 1;
            }
        }

        // Update average reduction in dB for monitoring
        if bin_count > 0 {
            let avg_gain = 1.0 - (total_reduction / bin_count as f32);
            self.avg_reduction_db = if avg_gain > EPSILON {
                -20.0 * avg_gain.log10()
            } else {
                60.0
            };
        }
    }
}
