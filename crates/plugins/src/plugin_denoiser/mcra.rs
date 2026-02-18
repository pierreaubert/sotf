// ============================================================================
// IMCRA (Improved Minimum Controlled Recursive Averaging) Noise Estimation
// ============================================================================
//
// Implements IMCRA with dual-window minimum tracking for robust noise power
// spectral density estimation from noisy signals.
//
// Improvements over basic MCRA:
// - Dual staggered minimum windows eliminate periodic blind spots
// - Multi-frame bootstrap for stable initialization
//
// Reference: Israel Cohen, "Noise Spectrum Estimation in Adverse Environments:
//            Improved Minima Controlled Recursive Averaging", 2003

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

/// Number of frames to average during the bootstrap phase.
/// During bootstrap, gains are unity (pass-through) while the noise
/// floor estimate converges from multiple frames.
/// 20 frames (~213ms at 48kHz/2048-sample FFT) gives the noise floor
/// enough time to converge before processing begins.
const BOOTSTRAP_FRAMES: usize = 20;

impl DenoiserPlugin {
    /// Orchestrate noise estimation: bootstrap phase then IMCRA.
    /// Returns `true` while any channel is still bootstrapping.
    pub(super) fn update_noise_estimation(&mut self) -> bool {
        let mut any_bootstrapping = false;

        for ch in 0..self.channels {
            if self.frame_counter[ch] < BOOTSTRAP_FRAMES {
                // Bootstrap: accumulate power into noise_psd
                for k in 0..self.spectrum_size {
                    let power = self.get_power_at_bin(ch, k);
                    self.noise_psd[ch][k] += power;
                }
                self.frame_counter[ch] += 1;

                if self.frame_counter[ch] >= BOOTSTRAP_FRAMES {
                    self.finalize_bootstrap(ch);
                } else {
                    any_bootstrapping = true;
                }
            } else {
                self.update_mcra(ch);
            }
        }

        any_bootstrapping
    }

    /// Finalize bootstrap by averaging accumulated power and initializing
    /// all MCRA state from the averaged noise estimate.
    fn finalize_bootstrap(&mut self, channel: usize) {
        let n = self.frame_counter[channel] as f32;
        for k in 0..self.spectrum_size {
            let avg = self.noise_psd[channel][k] / n;
            self.noise_psd[channel][k] = avg;
            self.smoothed_psd[channel][k] = avg;
            self.min_psd[channel][k] = avg;
            self.min_psd_b[channel][k] = avg;
            self.speech_presence[channel][k] = 0.0;
        }
    }

    /// Update IMCRA noise estimation for one channel.
    ///
    /// Algorithm:
    /// 1. Smooth power spectrum: S_tmp = α_s * S_tmp + (1-α_s) * |X|²
    /// 2. Dual-window minimum tracking (eliminates periodic blind spots)
    /// 3. Detect speech presence: I = 1 if S_tmp/S_min > δ else 0
    /// 4. Smooth speech probability: p = α_p * p + (1-α_p) * I
    /// 5. Update noise: σ_n² adaptive based on speech probability
    fn update_mcra(&mut self, channel: usize) {
        let alpha_s = self.mcra_alpha_s;
        let alpha_p = self.mcra_alpha_p;
        let delta = self.mcra_delta;
        let l = self.mcra_l;
        let half_l = l / 2;
        let frame = self.frame_counter[channel];

        // Track whether we're learning (quiet moment detected)
        let mut quiet_bins = 0;

        for k in 0..self.spectrum_size {
            let power = self.get_power_at_bin(channel, k);

            // Step 1: Update smoothed power spectral density
            let s_tmp = alpha_s * self.smoothed_psd[channel][k] + (1.0 - alpha_s) * power;
            self.smoothed_psd[channel][k] = s_tmp;

            // Step 2: IMCRA dual-window minimum tracking
            // Window A: resets at frames 0, L, 2L, ...
            if frame.is_multiple_of(l) {
                self.min_psd[channel][k] = s_tmp;
            } else {
                self.min_psd[channel][k] = self.min_psd[channel][k].min(s_tmp);
            }

            // Window B: resets at frames L/2, 3L/2, 5L/2, ...
            if frame % l == half_l {
                self.min_psd_b[channel][k] = s_tmp;
            } else {
                self.min_psd_b[channel][k] = self.min_psd_b[channel][k].min(s_tmp);
            }

            // Use minimum of both windows (ensures no blind spot at reset)
            let s_min = self.min_psd[channel][k]
                .min(self.min_psd_b[channel][k])
                .max(EPSILON);

            // Step 3: Compute speech presence indicator
            let s_r = s_tmp / s_min;
            let indicator = if s_r > delta { 1.0 } else { 0.0 };

            // Step 4: Smooth speech presence probability
            let p = alpha_p * self.speech_presence[channel][k] + (1.0 - alpha_p) * indicator;
            self.speech_presence[channel][k] = p;

            // Step 5: Update noise estimate with adaptive smoothing
            // When p is high (speech present), alpha_d → 1 → slow/no update
            // When p is low (speech absent), alpha_d → alpha_s → normal update
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
        self.min_psd_b[channel].fill(0.0);
        self.speech_presence[channel].fill(0.0);
        self.frame_counter[channel] = 0;
    }
}
