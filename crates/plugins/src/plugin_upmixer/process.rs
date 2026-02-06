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

        // Defensive check: ensure plugin has been initialized
        debug_assert!(
            self.sample_rate > 0 && self.fft_size > 0,
            "process_fft_block called before initialize()"
        );

        /*
                log::trace!(
                    "[UPMIXER] process_fft_block() start: fft_size={}, num_output_channels={}",
                    self.fft_size,
                    self.num_output_channels
                );
        */

        // Phase 1: Apply window and perform forward FFT
        self.apply_window_and_forward_fft(input);

        // 3E: Spectral flux transient detection
        // Replaces broadband HF energy ratio with spectral flux (sum of positive
        // magnitude increases) for more accurate onset detection.
        if self.bypass_transient_detection {
            self.hr_transient_env = 0.0;
        } else if self.enable_hr_direct {
            let spectrum_size = self.fft_size / 2 + 1;

            // Compute spectral flux: sum of positive magnitude increases
            let mut flux = 0.0_f32;
            for i in 0..spectrum_size {
                let l = self.freq_domain_left[i];
                let r = self.freq_domain_right[i];
                let current_mag = (l.norm_sqr() + r.norm_sqr()).sqrt();

                let prev_mag = if i < self.prev_magnitude_spectrum.len() {
                    self.prev_magnitude_spectrum[i]
                } else {
                    0.0
                };

                // Only count positive increases (onsets, not offsets)
                let diff = current_mag - prev_mag;
                if diff > 0.0 {
                    flux += diff;
                }

                // Store current magnitude for next frame
                if i < self.prev_magnitude_spectrum.len() {
                    self.prev_magnitude_spectrum[i] = current_mag;
                }
            }

            // Normalize flux by spectrum size
            flux /= spectrum_size as f32;

            // Smooth flux for normalization baseline
            let flux_smooth_alpha = 0.05_f32;
            self.spectral_flux_smooth += flux_smooth_alpha * (flux - self.spectral_flux_smooth);

            // Compute transient target: flux relative to smoothed baseline
            let transient_target = if self.spectral_flux_smooth > 1e-9 {
                let ratio = flux / self.spectral_flux_smooth;
                // Map ratio > 1 to transient envelope (0-1 range)
                ((ratio - 1.0) / 3.0).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Fast attack / slow release envelope
            let prev_env = self.hr_transient_env;
            let alpha_env = if transient_target > prev_env {
                0.4_f32 // Fast attack
            } else {
                0.15_f32 // Slow release
            };
            self.hr_transient_env = prev_env + alpha_env * (transient_target - prev_env);
        } else {
            self.hr_transient_env = 0.0;
        }

        // Dialogue Detection: analyze spectral centroid and temporal envelope
        let _dialogue_prob = self.detect_dialogue();

        // Phase 2: Frequency-domain processing (ERB Bands + PCA)
        self.process_frequency_domain_erb_bands();

        // Phase 3: Apply VBAP panning and inverse FFT
        // Calculate combined scaling factor for output
        // Guard against division by zero (defensive, fft_size should be validated in initialize)
        let fft_scale = if self.fft_size > 0 {
            1.0 / self.fft_size as f32
        } else {
            1.0 // Fallback, shouldn't happen
        };
        let cola_scale = 2.0; // COLA compensation for Hann window at 50% overlap
        let channel_normalization = 0.9 / 2.0_f32.sqrt(); // Prevent clipping
        let combined_scale = fft_scale * cola_scale * channel_normalization;

        // Guard against NaN/Inf from any prior processing
        let combined_scale = if combined_scale.is_finite() && combined_scale > 0.0 {
            combined_scale
        } else {
            1.0
        };

        self.apply_vbap_panning_and_inverse_fft();

        // Phase 4: Sub-harmonic synthesis (time domain)
        self.apply_subharmonic_synthesis();

        // Phase 5: Extract output and apply final scaling
        self.extract_output_and_scale(output, combined_scale);
    }
}
