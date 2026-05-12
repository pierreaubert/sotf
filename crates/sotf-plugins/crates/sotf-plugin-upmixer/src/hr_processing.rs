// ============================================================================
// High-Resolution Processing
// ============================================================================

use super::UpmixerPlugin;
use rustfft::num_complex::Complex;

impl UpmixerPlugin {
    fn hr_target_scale(&self) -> f32 {
        let hr_mix = (self.hr_transient_env * self.hr_sharpen.current() * self.hr_direct_envelope)
            .clamp(0.0, 1.0);

        if hr_mix < 0.01 || self.gain_front_direct.current() <= 0.0 {
            0.0
        } else {
            // Scale HR path relative to main: sqrt ratio avoids overpowering the
            // main path while still providing transient detail enhancement.
            // Also apply the 1/N overlap-add scaling factor for the HR path itself.
            // Multiply by sqrt(2) to compensate for the -3 dB headroom scale.
            let hr_ola_scale = std::f32::consts::SQRT_2 / self.hr_fft_size as f32;
            (self.fft_size as f32 / self.hr_fft_size as f32).sqrt() * hr_mix * hr_ola_scale
        }
    }

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

            // Process bins with a raised-cosine transition band to avoid brick-wall
            // Gibbs ringing. A hard cutoff at hf_cut creates pre/post echoes around
            // transients — precisely when the HR path is most active.
            // Transition region: [hf_cut - transition_bw, hf_cut], width = 8 bins.
            let transition_bw = 8.0 * freq_per_bin;
            self.hr_temp_freq_out.fill(Complex::new(0.0, 0.0));

            for i in 0..hr_spectrum_size {
                let freq = i as f32 * freq_per_bin;
                let gain = if freq <= hf_cut - transition_bw {
                    0.0
                } else if freq >= hf_cut {
                    1.0
                } else {
                    // Raised cosine: smoothly ramps from 0 to 1 over transition_bw
                    let t = (freq - (hf_cut - transition_bw)) / transition_bw;
                    0.5 - 0.5 * (std::f32::consts::PI * t).cos()
                };
                if gain > 0.0 {
                    let l = self.hr_freq_domain_left[i];
                    let r = self.hr_freq_domain_right[i];
                    self.hr_temp_freq_out[i] =
                        (l * panning_gain_left + r * panning_gain_right) * gain_scale * gain;
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

            // Matching sqrt-Hann synthesis window. The 1/N OLA normalization is
            // applied in mix_hr_output via hr_ola_scale.
            sotf_host::simd::window_mul_simd_inplace(
                &mut self.hr_time_out_channels[ch_idx],
                &self.hr_window,
            );
        }
    }

    /// Drain `num_frames` from the HR output ring buffer and mix into `output`.
    /// Operates in synchronized lockstep with the main path via the delay buffer.
    pub(super) fn mix_hr_output(&mut self, output: &mut [f32], num_frames: usize) {
        let drain = num_frames.min(self.hr_output_accumulator_fill);

        let nch = self.num_output_channels;
        let mask = self.hr_output_accumulator_mask;
        let target_scale = self.hr_target_scale();

        let mut scale = self.prev_hr_scale;
        let scale_step = (target_scale - scale) / drain.max(1) as f32;

        // Drain HR output and mix directly into main output buffer
        for i in 0..drain {
            scale += scale_step;
            let read_idx = (self.hr_output_read_position + i) & mask;
            let acc_base = read_idx * nch;
            let out_base = i * nch;

            for &ch in &self.cached_hr_active_channels {
                output[out_base + ch] += self.hr_output_accumulator[acc_base + ch] * scale;
                self.hr_output_accumulator[acc_base + ch] = 0.0;
            }
        }

        self.prev_hr_scale = target_scale;
        self.hr_output_read_position = (self.hr_output_read_position + drain) & mask;
        self.hr_output_accumulator_fill -= drain;
    }

    /// Drain HR output into a 2-channel binaural preview buffer.
    pub(super) fn mix_hr_output_binaural(&mut self, output: &mut [f32], num_frames: usize) {
        let drain = num_frames.min(self.hr_output_accumulator_fill);
        let nch = self.num_output_channels;
        let mask = self.hr_output_accumulator_mask;
        let target_scale = self.hr_target_scale();

        let mut scale = self.prev_hr_scale;
        let scale_step = (target_scale - scale) / drain.max(1) as f32;

        for i in 0..drain {
            scale += scale_step;
            let read_idx = (self.hr_output_read_position + i) & mask;
            let acc_base = read_idx * nch;
            let out_base = i * 2;
            let mut left = 0.0;
            let mut right = 0.0;

            for &ch in &self.cached_hr_active_channels {
                let sample = self.hr_output_accumulator[acc_base + ch];
                let (left_gain, right_gain) = self.binaural_preview_gains_for_channel(ch);
                left += sample * left_gain;
                right += sample * right_gain;
                self.hr_output_accumulator[acc_base + ch] = 0.0;
            }

            output[out_base] += left * scale;
            output[out_base + 1] += right * scale;
        }

        self.prev_hr_scale = target_scale;
        self.hr_output_read_position = (self.hr_output_read_position + drain) & mask;
        self.hr_output_accumulator_fill -= drain;
    }

    /// Process one HR FFT block and accumulate into the HR output ring buffer.
    pub(super) fn process_hr_block(&mut self, temp_input: &[f32]) {
        self.process_hr_fft(temp_input);

        let mask = self.hr_output_accumulator_mask;
        let nch = self.num_output_channels;
        let hr_hop = self.hr_fft_size / 2;
        let hr_ring_capacity = mask + 1;

        // Guard against ring buffer overflow
        debug_assert!(
            self.hr_output_accumulator_fill + hr_hop <= hr_ring_capacity,
            "HR ring buffer overflow: fill {} + hop {} > capacity {}",
            self.hr_output_accumulator_fill,
            hr_hop,
            hr_ring_capacity
        );
        if self.hr_output_accumulator_fill + hr_hop > hr_ring_capacity {
            return;
        }

        for i in 0..self.hr_fft_size {
            let write_idx = (self.hr_next_add_position + i) & mask;
            let acc_base = write_idx * nch;

            for &ch in &self.cached_hr_active_channels {
                self.hr_output_accumulator[acc_base + ch] += self.hr_time_out_channels[ch][i];
            }
        }

        self.hr_next_add_position = (self.hr_next_add_position + hr_hop) & mask;
        self.hr_output_accumulator_fill += hr_hop;
    }
}
