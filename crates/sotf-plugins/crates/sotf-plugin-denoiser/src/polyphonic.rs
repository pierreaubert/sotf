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

        // 6 dB threshold ~ signal is 4x noise power
        const SNR_THRESHOLD_LINEAR: f32 = 3.981_072; // 10^(6/10)

        let mut total_reduction = 0.0_f32;
        let mut bin_count = 0;

        for ch in 0..self.channels {
            // Pass 1: Compute instantaneous gate gain
            for k in 0..self.spectrum_size {
                let signal_power = self.get_power_at_bin(ch, k);
                let noise_power = self.get_effective_noise_power(ch, k);
                let snr = signal_power / noise_power.max(EPSILON);

                let target_gain = if snr > SNR_THRESHOLD_LINEAR {
                    1.0
                } else {
                    floor_linear
                };
                self.gain[ch][k] = target_gain;
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
