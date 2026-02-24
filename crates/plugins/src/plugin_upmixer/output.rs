// ============================================================================
// Output Processing
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::flush_denormals_inplace;

impl UpmixerPlugin {
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
        if self.safety_cap_db > 0.0 {
            let mut max_abs = 0.0_f32;
            for ch in 0..self.num_output_channels {
                if self.cached_is_height[ch] {
                    continue;
                }
                for i in 0..self.fft_size {
                    let v = self.time_out_channels[ch][i].abs();
                    if v > max_abs {
                        max_abs = v;
                    }
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

        let threshold = 0.95_f32;
        let inv_fft_size = 1.0 / self.fft_size as f32;

        for i in 0..self.fft_size {
            let idx = i * self.num_output_channels;

            // Step 1: Apply fixed STFT scale and block-level safety scale
            let t = i as f32 * inv_fft_size;
            let block_safety_scale = start_scale + t * (end_scale - start_scale);
            let base_scale = combined_scale * block_safety_scale;

            // Step 2: Detect peak across channels for this sample
            let mut peak = 0.0_f32;
            for ch in 0..self.num_output_channels {
                let v = (self.time_out_channels[ch][i] * base_scale).abs();
                if v > peak {
                    peak = v;
                }
            }

            // Step 3: Update per-sample limiter envelope
            let target_gr = if peak > threshold {
                threshold / peak
            } else {
                1.0
            };
            if target_gr < self.limiter_envelope {
                // Fast attack
                self.limiter_envelope =
                    target_gr + self.limiter_attack_coeff * (self.limiter_envelope - target_gr);
            } else {
                // Slow release
                self.limiter_envelope =
                    target_gr + self.limiter_release_coeff * (self.limiter_envelope - target_gr);
            }

            // Step 4: Apply final combined scale to all channels
            let final_scale = base_scale * self.limiter_envelope;
            for ch in 0..self.num_output_channels {
                output[idx + ch] = self.time_out_channels[ch][i] * final_scale;
            }
        }

        // Note: FTZ/DAZ CPU flags are set by the processing thread (enable_ftz_daz()),
        // so denormals are flushed to zero by the CPU automatically. No manual flush needed.
    }
}
