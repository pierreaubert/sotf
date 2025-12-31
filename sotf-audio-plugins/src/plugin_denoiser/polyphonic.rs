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
    /// This method:
    /// 1. Calculates SNR for each frequency bin using MCRA noise estimate
    /// 2. Applies a hard gate: if SNR > Threshold, Gain = 1.0, else Gain = Floor
    /// 3. Smooths gains with attack/release envelope to prevent clicking
    pub(super) fn calculate_polyphonic_gains(&mut self) {
        let floor_linear = 10.0_f32.powf(self.floor_db / 20.0);
        
        // Threshold: We consider something a "note" if it is significantly above the noise floor.
        // We use a fixed threshold of 6dB for now, or we could make it a parameter.
        // 6dB roughly corresponds to signal being 2x amplitude of noise (4x power).
        let snr_threshold_db = 6.0; 
        let snr_threshold_linear = 10.0_f32.powf(snr_threshold_db / 10.0);

        let mut total_reduction = 0.0_f32;
        let mut bin_count = 0;

        for ch in 0..self.channels {
            for k in 0..self.spectrum_size {
                 // Get signal and noise power
                let signal_power = self.get_power_at_bin(ch, k);
                let noise_power = self.get_noise_power(ch, k);

                let snr = signal_power / noise_power.max(EPSILON);
                
                // Detection logic:
                // If SNR is high, it's a note (or strong signal).
                // We apply a binary gate behavior (softened by the envelope follower later).
                let target_gain = if snr > snr_threshold_linear {
                    1.0
                } else {
                    floor_linear
                };

                // Store instantaneous gain
                self.gain[ch][k] = target_gain;

                // Apply temporal smoothing with attack/release
                // This converts the binary spectral gate into a smooth spectral expander/gate
                let prev_gain = self.smoothed_gain[ch][k];
                
                // Logic:
                // If target > prev (Note onset), use attack (fast)
                // If target < prev (Note release), use release (slow)
                let coeff = if target_gain > prev_gain {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };
                
                let smoothed = target_gain + coeff * (prev_gain - target_gain);
                self.smoothed_gain[ch][k] = smoothed;

                // Track average reduction for monitoring
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
