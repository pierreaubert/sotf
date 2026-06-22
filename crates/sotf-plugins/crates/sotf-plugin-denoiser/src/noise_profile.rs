// ============================================================================
// Noise Profile Capture
// ============================================================================
//
// Allows the user to capture a noise profile during a noise-only section.
// When active, the captured profile overrides the live MCRA noise estimation,
// providing more stable and accurate noise reduction.
//
// Usage:
// 1. Play a noise-only section (e.g. silence between tracks)
// 2. Trigger "Learn Noise" — accumulates noise power over ~1 second
// 3. Toggle "Use Profile" to switch between captured and live MCRA
// 4. "Clear Profile" resets back to live-only mode

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

impl DenoiserPlugin {
    /// Accumulate current frame's power into the learning accumulator
    pub(super) fn accumulate_noise_frame(&mut self) {
        for ch in 0..self.config.channels {
            for k in 0..self.config.spectrum_size {
                let power = self.get_power_at_bin(ch, k);
                self.noise_profile.learning_accumulator[ch][k] += power;
            }
        }
        self.noise_profile.learning_frames_count += 1;

        // Check if we've collected enough frames
        if self.noise_profile.learning_frames_count >= self.noise_profile.learning_frames_target {
            self.finalize_noise_profile();
        }
    }

    /// Finalize the noise profile by averaging accumulated frames.
    /// Writes into pre-allocated storage to avoid allocations on the audio thread.
    fn finalize_noise_profile(&mut self) {
        let count = self.noise_profile.learning_frames_count as f32;
        if count < 1.0 {
            return;
        }

        for ch in 0..self.config.channels {
            for k in 0..self.config.spectrum_size {
                self.noise_profile.noise_profile_storage[ch][k] =
                    self.noise_profile.learning_accumulator[ch][k] / count;
            }
        }

        self.noise_profile.has_noise_profile = true;
        self.noise_profile.use_captured_profile = true;
        self.noise_profile.is_learning = false;

        // Reset accumulator
        for ch in 0..self.config.channels {
            self.noise_profile.learning_accumulator[ch].fill(0.0);
        }
        self.noise_profile.learning_frames_count = 0;
    }

    /// Start the noise learning process.
    ///
    /// Also resets the live MCRA state for all channels so the noise
    /// floor re-enters the bootstrap phase and converges on the current
    /// noise environment from scratch.
    pub(super) fn start_learning(&mut self) {
        self.noise_profile.is_learning = true;
        self.noise_profile.learning_frames_count = 0;
        for ch in 0..self.config.channels {
            self.noise_profile.learning_accumulator[ch].fill(0.0);
            self.reset_mcra(ch);
        }
    }

    /// Clear the captured noise profile
    pub(super) fn clear_noise_profile(&mut self) {
        self.noise_profile.has_noise_profile = false;
        self.noise_profile.use_captured_profile = false;
        self.noise_profile.is_learning = false;
        self.noise_profile.learning_frames_count = 0;
        for ch in 0..self.config.channels {
            self.noise_profile.learning_accumulator[ch].fill(0.0);
        }
    }

    /// Get the effective noise power for a given bin.
    /// Returns captured profile if active and available, otherwise live MCRA.
    #[inline]
    pub(super) fn get_effective_noise_power(&self, channel: usize, bin: usize) -> f32 {
        if self.noise_profile.use_captured_profile && self.noise_profile.has_noise_profile {
            return self.noise_profile.noise_profile_storage[channel][bin].max(EPSILON);
        }
        self.get_noise_power(channel, bin)
    }

    /// Get the learning progress as a fraction (0.0 to 1.0)
    pub(super) fn learning_progress(&self) -> f32 {
        if !self.noise_profile.is_learning || self.noise_profile.learning_frames_target == 0 {
            return 0.0;
        }
        self.noise_profile.learning_frames_count as f32
            / self.noise_profile.learning_frames_target as f32
    }
}
