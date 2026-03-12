// ============================================================================
// Hiss Remover
// ============================================================================
//
// Targets stationary high-frequency noise (tape hiss, preamp hiss, etc.)
// by applying additional attenuation to frequency bins above a configurable
// cutoff where the signal-to-noise ratio is low.
//
// Unlike full-band Wiener denoising, the hiss remover only affects
// high-frequency content, preserving the low/mid range untouched.

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

impl DenoiserPlugin {
    /// Apply hiss removal to the gain array for one channel.
    ///
    /// For bins above `hiss_cutoff_bin`, compute a local SNR and apply
    /// additional attenuation proportional to `hiss_strength` when the
    /// SNR is below the threshold.
    pub(super) fn apply_hiss_removal(&mut self, channel: usize) {
        let cutoff_bin = self.hiss_cutoff_bin;
        let strength = self.hiss_strength;
        let threshold_linear = self.hiss_threshold_linear;

        for k in cutoff_bin..self.spectrum_size {
            let signal_power = self.get_power_at_bin(channel, k);
            let noise_power = self.get_effective_noise_power(channel, k).max(EPSILON);
            let snr = signal_power / noise_power;

            // Below threshold: attenuate proportional to strength and how far below threshold
            if snr < threshold_linear {
                // Ratio goes from 1.0 (at threshold) to 0.0 (at zero SNR)
                let ratio = snr / threshold_linear;
                // Gain reduction: blend between current gain and floor based on strength
                let hiss_gain = ratio + (1.0 - strength) * (1.0 - ratio);
                self.gain[channel][k] *= hiss_gain;
            }
        }
    }

    /// Convert hiss frequency to bin index. Call when sample_rate or fft_size changes.
    pub(super) fn update_hiss_cutoff_bin(&mut self) {
        let bin_freq = self.sample_rate as f32 / self.fft_size as f32;
        self.hiss_cutoff_bin = (self.hiss_frequency_hz / bin_freq).round() as usize;
        self.hiss_cutoff_bin = self
            .hiss_cutoff_bin
            .min(self.spectrum_size.saturating_sub(1));
    }

    /// Convert hiss threshold from dB to linear power ratio.
    pub(super) fn update_hiss_threshold_linear(&mut self) {
        self.hiss_threshold_linear = 10.0_f32.powf(self.hiss_threshold_db / 10.0);
    }
}
