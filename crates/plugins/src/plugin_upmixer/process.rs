// ============================================================================
// Main Processing Functions
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Process one FFT block using VBAP panning
    pub fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), self.fft_size * 2);
        assert_eq!(output.len(), self.fft_size * self.num_output_channels);

        debug_assert!(self.sample_rate > 0 && self.fft_size > 0);

        self.apply_window_and_forward_fft(input);

        if self.bypass_transient_detection {
            self.hr_transient_env = 0.0;
        } else if self.enable_hr_direct {
            let spectrum_size = self.fft_size / 2 + 1;
            let mut flux = 0.0_f32;
            for i in 0..spectrum_size {
                let l = self.freq_domain_left[i];
                let r = self.freq_domain_right[i];
                let current_power = l.norm_sqr() + r.norm_sqr();
                let prev_power = self.prev_magnitude_spectrum[i];
                let diff = current_power - prev_power;
                if diff > 0.0 {
                    flux += diff;
                }
                self.prev_magnitude_spectrum[i] = current_power;
            }

            flux /= spectrum_size as f32;
            self.spectral_flux_smooth += 0.05 * (flux - self.spectral_flux_smooth);

            let transient_target = if self.spectral_flux_smooth > 1e-9 {
                ((flux / self.spectral_flux_smooth - 1.0) / 3.0).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let alpha_env = if transient_target > self.hr_transient_env {
                0.4
            } else {
                0.15
            };
            self.hr_transient_env += alpha_env * (transient_target - self.hr_transient_env);
        } else {
            self.hr_transient_env = 0.0;
        }

        let _dialogue_prob = self.detect_dialogue();
        self.process_frequency_domain_erb_bands();

        let fft_scale = 1.0 / self.fft_size as f32;
        let combined_scale = fft_scale * 2.0 * (0.9 / 2.0_f32.sqrt()); // 2.0 for Hann COLA

        self.apply_vbap_panning_and_inverse_fft();
        self.apply_subharmonic_synthesis();
        self.extract_output_and_scale(output, combined_scale);
    }
}
