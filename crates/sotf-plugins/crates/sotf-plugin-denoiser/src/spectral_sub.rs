// ============================================================================
// Spectral Subtraction
// ============================================================================
//
// Classic spectral subtraction noise reduction. Computes gain as:
//   G(k) = max(1 - α · N(k)/S(k), β)^0.5
//
// where α is the oversubtraction factor and β is the spectral floor.
//
// Compared to Wiener filtering:
// - More aggressive noise removal
// - Can introduce "musical noise" artifacts if used alone
// - Works well combined with Wiener (takes minimum of both gains)
//
// Reference: Berouti, Schwartz, Makhoul, "Enhancement of Speech Corrupted
//            by Acoustic Noise", 1979

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

impl DenoiserPlugin {
    /// Apply spectral subtraction gains for a single channel.
    ///
    /// Combines with existing Wiener gains via minimum: the more
    /// conservative (lower) gain wins at each bin.
    pub(super) fn calculate_spectral_subtraction_gains_for_channel(&mut self, channel: usize) {
        let alpha = self.spectral_sub.spectral_sub_alpha;
        let beta = self.spectral_sub.spectral_sub_beta;
        let transparency = self.params.transparency;

        for k in 0..self.config.spectrum_size {
            let signal_power = self.get_power_at_bin(channel, k).max(EPSILON);
            let noise_power = self.get_effective_noise_power(channel, k);

            // Spectral subtraction: G² = max(1 - α·N/S, β)
            let subtracted = 1.0 - alpha * noise_power / signal_power;
            let gain_sq = subtracted.max(beta);
            let gain = gain_sq.sqrt();

            // Blend toward dry signal based on transparency
            let gain = gain + transparency * (1.0 - gain);

            // Combine with existing gain (minimum of Wiener and spectral sub)
            self.gains.gain[channel][k] = self.gains.gain[channel][k].min(gain);
        }
    }
}
