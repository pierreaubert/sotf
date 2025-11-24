// ============================================================================
// Main Processing Functions
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Process one FFT block using VBAP panning
    ///
    /// This is the main processing function that coordinates all the phases:
    /// 1. Apply window and forward FFT
    /// 2. Transient detection (for HR path)
    /// 3. Dialogue detection
    /// 4. Frequency domain processing (ERB bands + PCA)
    /// 5. VBAP panning and inverse FFT
    /// 6. Sub-harmonic synthesis
    /// 7. Extract output and apply final scaling
    pub fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes
        assert_eq!(input.len(), self.fft_size * 2); // stereo interleaved
        assert_eq!(output.len(), self.fft_size * self.num_output_channels); // variable channels

        /*
                log::trace!(
                    "[UPMIXER] process_fft_block() start: fft_size={}, num_output_channels={}",
                    self.fft_size,
                    self.num_output_channels
                );
        */

        // Phase 1: Apply window and perform forward FFT
        self.apply_window_and_forward_fft(input);

        // High-frequency transient detector for HR direct-path crossfade.
        if self.enable_hr_direct {
            let spectrum_size = self.fft_size / 2 + 1;
            let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
            let hf_start = self.bandpass_hz.max(1000.0);

            let mut energy = 0.0_f32;
            let mut count = 0usize;
            for i in 0..spectrum_size {
                let freq = i as f32 * freq_per_bin;
                if freq >= hf_start {
                    let l = self.freq_domain_left[i];
                    let r = self.freq_domain_right[i];
                    energy += l.norm_sqr() + r.norm_sqr();
                    count += 1;
                }
            }
            if count > 0 {
                energy /= count as f32;
            } else {
                energy = 0.0;
            }

            if self.hr_energy_smooth <= 0.0 {
                self.hr_energy_smooth = energy;
                self.hr_transient_env = 0.0;
            } else {
                let prev_smooth = self.hr_energy_smooth;
                let prev_smooth_clamped = prev_smooth.max(1e-9);
                let ratio = (energy / prev_smooth_clamped).max(0.0);

                let attack_e = 0.5_f32;
                let release_e = 0.1_f32;
                let alpha_e = if energy > prev_smooth {
                    attack_e
                } else {
                    release_e
                };
                self.hr_energy_smooth = prev_smooth + alpha_e * (energy - prev_smooth);

                let ratio_clamped = ratio.clamp(1.0, 4.0);
                let transient_target = if ratio_clamped > 1.0 {
                    (ratio_clamped - 1.0) / 3.0
                } else {
                    0.0
                };

                let prev_env = self.hr_transient_env;
                let attack_env = 0.8_f32;
                let release_env = 0.3_f32;
                let alpha_env = if transient_target > prev_env {
                    attack_env
                } else {
                    release_env
                };
                self.hr_transient_env = prev_env + alpha_env * (transient_target - prev_env);
            }
        } else {
            self.hr_transient_env = 0.0;
        }

        // Dialogue Detection: analyze spectral centroid and temporal envelope
        let _dialogue_prob = self.detect_dialogue();

        // Phase 2: Frequency-domain processing (ERB Bands + PCA)
        self.process_frequency_domain_erb_bands();

        // Phase 3: Apply VBAP panning and inverse FFT
        // Calculate combined scaling factor for output
        let fft_scale = 1.0 / self.fft_size as f32;
        let cola_scale = 2.0; // COLA compensation for Hann window at 50% overlap
        let channel_normalization = 0.9 / 2.0_f32.sqrt(); // Prevent clipping
        let combined_scale = fft_scale * cola_scale * channel_normalization;

        self.apply_vbap_panning_and_inverse_fft();

        // Phase 4: Sub-harmonic synthesis (time domain)
        self.apply_subharmonic_synthesis();

        // Phase 5: Extract output and apply final scaling
        self.extract_output_and_scale(output, combined_scale);
    }
}
