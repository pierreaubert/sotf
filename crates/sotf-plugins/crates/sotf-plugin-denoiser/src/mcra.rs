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
/// 5 frames (~53ms at 48kHz/2048-sample FFT) balances fast startup
/// with stable noise estimation.
const BOOTSTRAP_FRAMES: usize = 5;

impl DenoiserPlugin {
    /// Orchestrate noise estimation: bootstrap phase then IMCRA.
    /// Returns `true` while any channel is still bootstrapping.
    pub(super) fn update_noise_estimation(&mut self) -> bool {
        let mut any_bootstrapping = false;

        for ch in 0..self.config.channels {
            if self.mcra.frame_counter[ch] < BOOTSTRAP_FRAMES {
                // Bootstrap: accumulate power into noise_psd
                for k in 0..self.config.spectrum_size {
                    let power = self.get_power_at_bin(ch, k);
                    self.mcra.noise_psd[ch][k] += power;
                }
                self.mcra.frame_counter[ch] += 1;

                if self.mcra.frame_counter[ch] >= BOOTSTRAP_FRAMES {
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
        let n = self.mcra.frame_counter[channel] as f32;
        // Minimum noise floor: -60 dB power. Prevents division-by-near-zero
        // in Wiener gain when bootstrap captured mostly silence.
        let min_noise_power = 1e-6_f32;
        for k in 0..self.config.spectrum_size {
            let avg = (self.mcra.noise_psd[channel][k] / n).max(min_noise_power);
            self.mcra.noise_psd[channel][k] = avg;
            self.mcra.smoothed_psd[channel][k] = avg;
            self.mcra.min_psd[channel][k] = avg;
            self.mcra.min_psd_b[channel][k] = avg;
            self.mcra.speech_presence[channel][k] = 0.0;
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
        let alpha_s = self.mcra.mcra_alpha_s;
        let alpha_p = self.mcra.mcra_alpha_p;
        let delta = self.mcra.mcra_delta;
        let l = self.mcra.mcra_l;
        let half_l = l / 2;
        let frame = self.mcra.frame_counter[channel];

        // Hoist loop-invariant frame boundary checks out of the per-bin loop
        let reset_window_a = frame.is_multiple_of(l);
        let reset_window_b = frame % l == half_l;

        // Use previous frame's learning_active as fast-adapt trigger
        // (avoids chicken-and-egg: we need quiet_ratio before the loop)
        let use_fast_adapt = self.ui.learning_active;
        // Track whether we're learning (quiet moment detected)
        let mut quiet_bins = 0;

        for k in 0..self.config.spectrum_size {
            let power = self.get_power_at_bin(channel, k);

            // Step 1: Update smoothed power spectral density
            let s_tmp = alpha_s * self.mcra.smoothed_psd[channel][k] + (1.0 - alpha_s) * power;
            self.mcra.smoothed_psd[channel][k] = s_tmp;

            // Step 2: IMCRA dual-window minimum tracking
            // Window A: resets at frames 0, L, 2L, ...
            if reset_window_a {
                self.mcra.min_psd[channel][k] = s_tmp;
            } else {
                self.mcra.min_psd[channel][k] = self.mcra.min_psd[channel][k].min(s_tmp);
            }

            // Window B: resets at frames L/2, 3L/2, 5L/2, ...
            if reset_window_b {
                self.mcra.min_psd_b[channel][k] = s_tmp;
            } else {
                self.mcra.min_psd_b[channel][k] = self.mcra.min_psd_b[channel][k].min(s_tmp);
            }

            // Use minimum of both windows (ensures no blind spot at reset)
            let s_min = self.mcra.min_psd[channel][k]
                .min(self.mcra.min_psd_b[channel][k])
                .max(EPSILON);

            // Step 3: Compute speech presence indicator
            let s_r = s_tmp / s_min;
            let indicator = if s_r > delta { 1.0 } else { 0.0 };

            // Step 4: Smooth speech presence probability
            let p = alpha_p * self.mcra.speech_presence[channel][k] + (1.0 - alpha_p) * indicator;
            self.mcra.speech_presence[channel][k] = p;

            // Step 5: Update noise estimate with adaptive smoothing
            // When p is high (speech present), alpha_d → 1 → slow/no update
            // When p is low (speech absent), alpha_d → alpha_s → normal update
            // Fast adaptation: when previous frame was mostly quiet (learning_active),
            // use 2x faster tracking for low-speech bins to converge ~265ms vs ~530ms.
            let mut alpha_d = alpha_s + (1.0 - alpha_s) * p;
            if use_fast_adapt && p < 0.3 {
                alpha_d /= 2.0; // 2x faster tracking
            }
            let noise_est = alpha_d * self.mcra.noise_psd[channel][k] + (1.0 - alpha_d) * power;
            self.mcra.noise_psd[channel][k] = noise_est;

            // Count quiet bins for learning indicator
            if p < 0.5 {
                quiet_bins += 1;
            }
        }

        // Update learning indicator (more than half the bins are quiet)
        self.ui.learning_active = quiet_bins > self.config.spectrum_size / 2;

        // Increment frame counter
        self.mcra.frame_counter[channel] += 1;
    }

    /// Get estimated noise power at a specific bin
    #[inline]
    pub(super) fn get_noise_power(&self, channel: usize, bin: usize) -> f32 {
        self.mcra.noise_psd[channel][bin].max(EPSILON)
    }

    /// Reset MCRA state for a channel
    pub(super) fn reset_mcra(&mut self, channel: usize) {
        self.mcra.noise_psd[channel].fill(0.0);
        self.mcra.smoothed_psd[channel].fill(0.0);
        self.mcra.min_psd[channel].fill(0.0);
        self.mcra.min_psd_b[channel].fill(0.0);
        self.mcra.speech_presence[channel].fill(0.0);
        self.mcra.frame_counter[channel] = 0;
    }
}
