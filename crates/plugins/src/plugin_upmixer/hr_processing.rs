// ============================================================================
// High-Resolution Processing
// ============================================================================

use super::UpmixerPlugin;
use rustfft::num_complex::Complex;

impl UpmixerPlugin {
    /// Process one high-resolution FFT block for transient enhancement
    ///
    /// This function performs:
    /// 1. Windowing and forward FFT (shorter FFT for better time resolution)
    /// 2. High-frequency direct-path extraction (above bandpass_hz)
    /// 3. Per-channel VBAP panning (front speakers only)
    /// 4. Inverse FFT and scaling
    ///
    /// The HR path is used to enhance transient reproduction by processing
    /// with higher time resolution (smaller FFT) than the main path.
    pub(super) fn process_hr_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes: stereo interleaved input, variable output channels
        assert_eq!(input.len(), self.hr_fft_size * 2);
        assert_eq!(output.len(), self.hr_fft_size * self.num_output_channels);

        output.fill(0.0);

        // 1. Copy input to HR time-domain buffers and apply HR analysis window
        for i in 0..self.hr_fft_size {
            let idx = i * 2;
            let window_val = self.hr_window[i];
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

        // 4. Inverse FFT per-channel and write time-domain output
        let fft_scale = 1.0 / self.hr_fft_size as f32;
        let cola_scale = 2.0; // Hann with 50% overlap
        let channel_normalization = 0.5; // Conservative mix factor for HR path
        let combined_scale = fft_scale * cola_scale * channel_normalization;

        let gain_front_direct = self.gain_front_direct.current();
        if gain_front_direct <= 0.0 {
            return;
        }

        for ch_idx in 0..self.num_output_channels {
            let speaker = &self.speaker_config.speakers[ch_idx];
            if speaker.is_lfe || speaker.elevation > 10.0 || speaker.azimuth.abs() >= 80.0 {
                continue;
            }

            let is_center = speaker.label == "C";
            let panning_gain_left = self.panning_gains_left[ch_idx];
            let panning_gain_right = self.panning_gains_right[ch_idx];

            let mut gain_scale = gain_front_direct;
            if is_center {
                let spread = self.center_spread.clamp(0.0, 1.0);
                gain_scale *= 1.0 - spread;
            }

            if gain_scale == 0.0 {
                continue;
            }

            // Optimization: Process bins only above cutoff and use gain_scale
            self.hr_temp_freq_out.fill(Complex::new(0.0, 0.0));
            
            for i in 0..hr_spectrum_size {
                let freq = i as f32 * freq_per_bin;
                if freq > hf_cut {
                    let l = self.hr_freq_domain_left[i];
                    let r = self.hr_freq_domain_right[i];
                    self.hr_temp_freq_out[i] = (l * panning_gain_left + r * panning_gain_right) * gain_scale;
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

        // Re-interleave HR channels into the output block
        // Optimization: iterate over frames first for sequential writes to output
        for i in 0..self.hr_fft_size {
            let out_idx = i * self.num_output_channels;
            for ch_idx in 0..self.num_output_channels {
                // Only front channels have data, others were cleared by fill(0.0) at start
                // We could optimize further by only iterating over active HR channels
                output[out_idx + ch_idx] += self.hr_time_out_channels[ch_idx][i] * combined_scale;
            }
        }
    }
}
