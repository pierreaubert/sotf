// ============================================================================
// Main Processing Functions
// ============================================================================

use super::UpmixerPlugin;

/// Attack alpha for spectral flux baseline (rises with singing, ~150ms)
const SPECTRAL_FLUX_ATTACK_ALPHA: f32 = 0.15;
/// Release alpha for spectral flux baseline (slow decay during rests)
const SPECTRAL_FLUX_RELEASE_ALPHA: f32 = 0.05;
/// Divisor for transient ratio normalization (flux/baseline - 1) / DIVISOR
const TRANSIENT_RATIO_DIVISOR: f32 = 3.0;
/// Attack alpha for HR transient envelope follower
const HR_TRANSIENT_ATTACK_ALPHA: f32 = 0.4;
/// Release alpha for HR transient envelope follower
const HR_TRANSIENT_RELEASE_ALPHA: f32 = 0.15;

impl UpmixerPlugin {
    /// Process one FFT block using VBAP panning
    pub fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), self.core.fft_size * 2);
        assert_eq!(
            output.len(),
            self.core.fft_size * self.core.num_output_channels
        );

        debug_assert!(self.core.sample_rate > 0 && self.core.fft_size > 0);

        self.apply_window_and_forward_fft(input);

        if self.params.bypass_transient_detection {
            self.hr_state.hr_transient_env = 0.0;
            self.height.height_transient_env_slow = 0.0;
        } else if self.params.enable_hr_direct || self.hr_state.hr_direct_envelope > 0.0 {
            let spectrum_size = self.core.fft_size / 2 + 1;
            let mut flux = 0.0_f32;
            for i in 0..spectrum_size {
                let l = self.main_buffers.freq_domain_left[i];
                let r = self.main_buffers.freq_domain_right[i];
                let current_power = l.norm_sqr() + r.norm_sqr();
                let prev_power = self.hr_state.prev_power_spectrum[i];
                let diff = current_power - prev_power;
                if diff > 0.0 {
                    flux += diff;
                }
                self.hr_state.prev_power_spectrum[i] = current_power;
            }

            flux /= spectrum_size as f32;
            // Bootstrap: on the very first frame with signal, seed the
            // baseline so the ratio doesn't spike to infinity.
            if self.hr_state.spectral_flux_smooth < 1e-12 && flux > 0.0 {
                self.hr_state.spectral_flux_smooth = flux;
            }
            let flux_alpha = if flux > self.hr_state.spectral_flux_smooth {
                SPECTRAL_FLUX_ATTACK_ALPHA
            } else {
                SPECTRAL_FLUX_RELEASE_ALPHA
            };
            self.hr_state.spectral_flux_smooth +=
                flux_alpha * (flux - self.hr_state.spectral_flux_smooth);

            let transient_target = if self.hr_state.spectral_flux_smooth > 1e-9 {
                ((flux / self.hr_state.spectral_flux_smooth - 1.0) / TRANSIENT_RATIO_DIVISOR)
                    .clamp(0.0, 1.0)
            } else {
                0.0
            };

            let alpha_env = if transient_target > self.hr_state.hr_transient_env {
                HR_TRANSIENT_ATTACK_ALPHA
            } else {
                HR_TRANSIENT_RELEASE_ALPHA
            };
            self.hr_state.hr_transient_env +=
                alpha_env * (transient_target - self.hr_state.hr_transient_env);

            // Slow envelope for height gain modulation — same attack, slower release
            // to avoid rapid gain pumping on sustained tonal content
            let alpha_height = if transient_target > self.height.height_transient_env_slow {
                HR_TRANSIENT_ATTACK_ALPHA
            } else {
                0.03
            };
            self.height.height_transient_env_slow +=
                alpha_height * (transient_target - self.height.height_transient_env_slow);
        } else {
            self.hr_state.hr_transient_env = 0.0;
            self.height.height_transient_env_slow = 0.0;
        }

        let _dialogue_prob = self.detect_dialogue();
        self.process_frequency_domain_erb_bands();

        // 50% overlap sqrt-Hann analysis/synthesis multiplies to a Hann
        // COLA sum of 1.0; realfft inverse is unnormalized, so base scale = 1/N.
        // Multiply by sqrt(2) to compensate for the -3 dB headroom scale
        // (1/sqrt(2)) applied to the input in apply_window_and_forward_fft.
        let combined_scale = std::f32::consts::SQRT_2 / self.core.fft_size as f32;

        self.apply_vbap_panning_and_inverse_fft();
        self.apply_subharmonic_synthesis();

        self.extract_output_and_scale(output, combined_scale);
    }
}
