// ============================================================================
// MCRA (Minimum Controlled Recursive Averaging) Noise Estimation
// ============================================================================
//
// MCRA is an algorithm for estimating the noise power spectral density
// from a noisy signal. It tracks the minimum power spectral density over
// time and uses speech presence probability to update the noise estimate
// only when speech is likely absent.
//
// Reference: Israel Cohen, "Noise Spectrum Estimation in Adverse Environments:
//            Improved Minima Controlled Recursive Averaging", 2003

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

impl DenoiserPlugin {
    /// Initialize MCRA state for all channels
    /// Called on first few frames to bootstrap noise estimate
    pub(super) fn initialize_mcra_from_frame(&mut self, channel: usize) {
        // Initialize noise estimate from current power spectrum
        // This assumes the first few frames are noise-only (common startup assumption)
        for k in 0..self.spectrum_size {
            let power = self.get_power_at_bin(channel, k);
            self.noise_psd[channel][k] = power;
            self.smoothed_psd[channel][k] = power;
            self.min_psd[channel][k] = power;
            self.speech_presence[channel][k] = 0.0;
        }
    }

    /// Update MCRA noise estimation for one channel
    ///
    /// Algorithm:
    /// 1. Smooth power spectrum: S_tmp = α_s * S_tmp + (1-α_s) * |X|²
    /// 2. Track minimum over L frames: S_min = min(S_min, S_tmp)
    /// 3. Detect speech presence: I = 1 if S_tmp/S_min > δ else 0
    /// 4. Smooth speech probability: p = α_p * p + (1-α_p) * I
    /// 5. Update noise: σ_n² adaptive based on speech probability
    pub(super) fn update_mcra(&mut self, channel: usize) {
        let alpha_s = self.mcra_alpha_s;
        let alpha_p = self.mcra_alpha_p;
        let delta = self.mcra_delta;
        let l = self.mcra_l;

        // Track whether we're learning (quiet moment detected)
        let mut quiet_bins = 0;

        for k in 0..self.spectrum_size {
            let power = self.get_power_at_bin(channel, k);

            // Step 1: Update smoothed power spectral density
            let s_tmp = alpha_s * self.smoothed_psd[channel][k] + (1.0 - alpha_s) * power;
            self.smoothed_psd[channel][k] = s_tmp;

            // Step 2: Track minimum (reset every L frames)
            if self.frame_counter[channel].is_multiple_of(l) {
                self.min_psd[channel][k] = s_tmp;
            } else {
                self.min_psd[channel][k] = self.min_psd[channel][k].min(s_tmp);
            }

            // Step 3: Compute speech presence indicator
            let s_min = self.min_psd[channel][k].max(EPSILON);
            let s_r = s_tmp / s_min;
            let indicator = if s_r > delta { 1.0 } else { 0.0 };

            // Step 4: Smooth speech presence probability
            let p = alpha_p * self.speech_presence[channel][k] + (1.0 - alpha_p) * indicator;
            self.speech_presence[channel][k] = p;

            // Step 5: Update noise estimate with adaptive smoothing
            // When p is high (speech present), alpha_d approaches 1 -> slow/no update
            // When p is low (speech absent), alpha_d approaches alpha_s -> normal update
            let alpha_d = alpha_s + (1.0 - alpha_s) * p;
            let noise_est = alpha_d * self.noise_psd[channel][k] + (1.0 - alpha_d) * power;
            self.noise_psd[channel][k] = noise_est;

            // Count quiet bins for learning indicator
            if p < 0.5 {
                quiet_bins += 1;
            }
        }

        // Update learning indicator (more than half the bins are quiet)
        self.learning_active = quiet_bins > self.spectrum_size / 2;

        // Increment frame counter
        self.frame_counter[channel] += 1;
    }

    /// Get estimated noise power at a specific bin
    #[inline]
    pub(super) fn get_noise_power(&self, channel: usize, bin: usize) -> f32 {
        self.noise_psd[channel][bin].max(EPSILON)
    }

    /// Reset MCRA state for a channel
    pub(super) fn reset_mcra(&mut self, channel: usize) {
        self.noise_psd[channel].fill(0.0);
        self.smoothed_psd[channel].fill(0.0);
        self.min_psd[channel].fill(0.0);
        self.speech_presence[channel].fill(0.0);
        self.frame_counter[channel] = 0;
    }

    /// Check if noise estimation is still in initialization phase
    #[inline]
    pub(super) fn is_initializing(&self, channel: usize) -> bool {
        // Consider first L frames as initialization phase
        self.frame_counter[channel] < self.mcra_l
    }
}
