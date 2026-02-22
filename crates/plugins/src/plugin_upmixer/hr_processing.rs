// ============================================================================
// High-Resolution Processing
// ============================================================================

use super::UpmixerPlugin;
use rustfft::num_complex::Complex;

impl UpmixerPlugin {
    /// Run HR FFT processing: window, forward FFT, HF filtering, IFFT per channel.
    /// Populates `hr_time_out_channels[ch]` with the per-channel time-domain results.
    /// Does NOT scale or mix — the caller handles that.
    fn process_hr_fft(&mut self, input: &[f32]) {
        // 1. Copy input to HR time-domain buffers and apply HR analysis window
        // Apply the same -3 dB headroom scale (1/sqrt(2)) as the main path (fft.rs)
        let headroom_scale = std::f32::consts::FRAC_1_SQRT_2;
        for i in 0..self.hr_fft_size {
            let idx = i * 2;
            let window_val = self.hr_window[i] * headroom_scale;
            self.hr_time_domain_left[i] = input[idx] * window_val;
            self.hr_time_domain_right[i] = input[idx + 1] * window_val;
        }

        // 2. Forward FFT (Real->Complex)
        self.hr_fft_forward
            .process(&mut self.hr_time_domain_left, &mut self.hr_freq_domain_left)
            .unwrap();
        self.hr_fft_forward
            .process(
                &mut self.hr_time_domain_right,
                &mut self.hr_freq_domain_right,
            )
            .unwrap();

        // 3. Frequency-dependent processing for HF direct path only
        let freq_per_bin = self.sample_rate as f32 / self.hr_fft_size as f32;
        let hf_cut = self.bandpass_hz.max(1000.0);
        let hr_spectrum_size = self.hr_fft_size / 2 + 1;

        let gain_front_direct = self.gain_front_direct.current();

        // Only process front, non-LFE, non-height channels (cached during build)
        for &ch_idx in &self.cached_hr_active_channels {
            let is_center = self.cached_is_center[ch_idx];
            let panning_gain_left = self.panning_gains_left[ch_idx];
            let panning_gain_right = self.panning_gains_right[ch_idx];

            let mut gain_scale = gain_front_direct;
            if is_center {
                let spread = self.center_spread.current();
                gain_scale *= 1.0 - spread;
            }

            if gain_scale == 0.0 {
                // Zero out this channel's HR output so stale data isn't mixed in
                self.hr_time_out_channels[ch_idx].fill(0.0);
                continue;
            }

            // Process bins only above cutoff
            self.hr_temp_freq_out.fill(Complex::new(0.0, 0.0));

            for i in 0..hr_spectrum_size {
                let freq = i as f32 * freq_per_bin;
                if freq > hf_cut {
                    let l = self.hr_freq_domain_left[i];
                    let r = self.hr_freq_domain_right[i];
                    self.hr_temp_freq_out[i] =
                        (l * panning_gain_left + r * panning_gain_right) * gain_scale;
                }
            }

            if hr_spectrum_size > 0 {
                self.hr_temp_freq_out[0].im = 0.0;
                self.hr_temp_freq_out[hr_spectrum_size - 1].im = 0.0;
            }

            self.hr_fft_inverse
                .process(
                    &mut self.hr_temp_freq_out,
                    &mut self.hr_time_out_channels[ch_idx],
                )
                .unwrap();
        }
    }

    /// Mix HR results into the main time_out_channels before output scaling.
    /// This ensures the safety cap in extract_output_and_scale() accounts for
    /// the combined main+HR energy, preventing uncontrolled peaks.
    ///
    /// Must be called after apply_vbap_panning_and_inverse_fft() and before
    /// extract_output_and_scale().
    pub(super) fn apply_hr_enhancement(&mut self, input: &[f32]) {
        let hr_mix = (self.hr_transient_env
            * self.hr_sharpen.current()
            * self.hr_direct_envelope)
            .clamp(0.0, 1.0);
        if hr_mix < 0.01 || self.gain_front_direct.current() <= 0.0 {
            return;
        }

        let center = (self.fft_size - self.hr_fft_size) / 2;
        let start = center * 2;
        let end = start + self.hr_fft_size * 2;
        if end > input.len() {
            return;
        }

        let hr_input = &input[start..end];
        self.process_hr_fft(hr_input);

        // Scale to match main path: (1/hr_fft_size) * 2.0 * (0.9/sqrt(2))
        // This matches the main path's combined_scale from process.rs:70-71
        // The result is then scaled by hr_mix to blend with the main path.
        // Note: the main path's combined_scale is applied later in extract_output_and_scale,
        // so we must apply the HR-equivalent scale here to get unity-matched levels.
        let scale = (self.fft_size as f32 / self.hr_fft_size as f32) * hr_mix;

        for &ch in &self.cached_hr_active_channels {
            for i in 0..self.hr_fft_size {
                self.time_out_channels[ch][center + i] +=
                    self.hr_time_out_channels[ch][i] * scale;
            }
        }
    }
}
