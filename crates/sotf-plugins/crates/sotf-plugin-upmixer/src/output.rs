// ============================================================================
// Output Processing
// ============================================================================

use super::UpmixerPlugin;
use sotf_host::simd::flush_denormals_inplace;

impl UpmixerPlugin {
    /// Apply the safety cap to final emitted samples after overlap-add and HR mixing.
    ///
    /// The per-FFT cap in `extract_output_and_scale` limits individual synthesis
    /// blocks, but overlapping blocks can still sum above the cap in the output
    /// accumulator. This final pass caps the real signal that leaves `process()`.
    #[inline]
    pub(super) fn apply_final_safety_cap(&mut self, output: &mut [f32], num_frames: usize) {
        if self.safety_cap_db < 0.0 || num_frames == 0 || self.num_output_channels == 0 {
            return;
        }

        let sample_count = (num_frames * self.num_output_channels).min(output.len());
        let samples = &mut output[..sample_count];
        if samples.is_empty() {
            return;
        }

        let max_abs = sotf_host::simd::find_max_abs_simd(samples);
        let mut target_scale = 1.0_f32;
        if max_abs.is_finite() && max_abs > self.safety_cap_linear && max_abs > 0.0 {
            target_scale = self.safety_cap_linear / max_abs;
        }

        // Attack immediately to guarantee the emitted chunk is capped. Release
        // slowly so recovery does not pump between adjacent host buffers.
        let release_coeff = 0.02_f32;
        let applied_scale = if target_scale < self.final_safety_scale {
            target_scale
        } else {
            self.final_safety_scale + release_coeff * (target_scale - self.final_safety_scale)
        };

        self.final_safety_scale = applied_scale.clamp(0.0, 1.0);
        if (self.final_safety_scale - 1.0).abs() > 1e-6 {
            sotf_host::simd::scale_add_simd_inplace(samples, self.final_safety_scale);
        }
    }

    /// Phase 5: Extract real parts from time domain and apply final scaling
    #[inline]
    pub(super) fn extract_output_and_scale(&mut self, output: &mut [f32], combined_scale: f32) {
        // ... (skipping some comments)
        let taper_len = self.edge_taper_table.len();
        for ch in 0..self.num_output_channels {
            // Flush denormals in the time-domain buffers before scaling/limiting.
            // This prevents performance spikes and keeps the peak detector accurate.
            flush_denormals_inplace(&mut self.time_out_channels[ch]);

            let speaker = &self.speaker_config.speakers[ch];
            if speaker.elevation > 10.0 {
                let taper_end = taper_len.min(self.fft_size / 2);
                for i in 0..taper_end {
                    let fade = self.edge_taper_table[i];
                    self.time_out_channels[ch][i] *= fade;
                    self.time_out_channels[ch][self.fft_size - 1 - i] *= fade;
                }
            }
        }

        // Safety cap: optionally reduce overall gain so that the block peak does not
        // exceed safety_cap_db (in dB) above unit amplitude.
        // Exclude height channels from peak detection: the height_band_gains spectral
        // mask (values down to 0.02) creates time-domain ringing after IFFT, producing
        // erratic peaks that would modulate the safety cap and cause scratchiness in
        // the bed channels (L/R/C/surround).
        let mut target_safety_scale = 1.0_f32;
        if self.safety_cap_db >= 0.0 {
            let mut max_abs = 0.0_f32;
            for ch in 0..self.num_output_channels {
                if self.cached_is_height[ch] {
                    continue;
                }
                let ch_max = sotf_host::simd::find_max_abs_simd(&self.time_out_channels[ch]);
                if ch_max > max_abs {
                    max_abs = ch_max;
                }
            }

            if max_abs > 0.0 {
                let cap_linear = self.safety_cap_linear;
                let effective_peak = max_abs * combined_scale;
                // Guard against division by zero or negative values (defensive)
                if effective_peak > 0.0 && effective_peak.is_finite() && effective_peak > cap_linear
                {
                    target_safety_scale = cap_linear / effective_peak;
                }
            }
        }

        // Smooth safety scale changes to avoid clicks and pumping artifacts.
        // Use asymmetric smoothing: fast attack (protect against clipping) but slow release.
        let attack_coeff = 0.5_f32; // Fast attack to catch peaks
        let release_coeff = 0.05_f32; // Slow release to avoid pumping
        let smoothing = if target_safety_scale < self.prev_safety_scale {
            attack_coeff // Going down (reducing gain) - fast
        } else {
            release_coeff // Going up (restoring gain) - slow
        };

        // Calculate the target scale for the end of this block
        let end_scale =
            self.prev_safety_scale + smoothing * (target_safety_scale - self.prev_safety_scale);

        let start_scale = self.prev_safety_scale;
        self.prev_safety_scale = end_scale;

        for i in 0..self.fft_size {
            let idx = i * self.num_output_channels;
            let t = i as f32 / self.fft_size as f32;
            let block_safety_scale = start_scale + t * (end_scale - start_scale);
            let final_scale = combined_scale * block_safety_scale;
            for ch in 0..self.num_output_channels {
                output[idx + ch] = self.time_out_channels[ch][i] * final_scale;
            }
        }

        // Note: FTZ/DAZ CPU flags are set by the processing thread (enable_ftz_daz()),
        // so denormals are flushed to zero by the CPU automatically. No manual flush needed.
    }
}
