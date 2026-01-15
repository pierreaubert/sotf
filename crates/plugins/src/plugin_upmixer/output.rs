// ============================================================================
// Output Processing
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::flush_denormals_inplace;

impl UpmixerPlugin {
    /// Phase 5: Extract real parts from time domain and apply final scaling
    #[inline]
    pub(super) fn extract_output_and_scale(&mut self, output: &mut [f32], combined_scale: f32) {
        // Note: With Hann window at 50% hop size, COLA (Constant Overlap-Add) is achieved by:
        // 1. Applying window ONCE during analysis (before FFT)
        // 2. Overlap-add with hop_size = fft_size/2
        // Applying window again here would break COLA and cause amplitude modulation artifacts
        // Safety cap: optionally reduce overall gain so that the block peak does not
        // exceed safety_cap_db (in dB) above unit amplitude.
        let mut target_safety_scale = 1.0_f32;
        if self.safety_cap_db > 0.0 {
            let mut max_abs = 0.0_f32;
            for ch in 0..self.num_output_channels {
                for i in 0..self.fft_size {
                    let v = self.time_out_channels[ch][i].abs();
                    if v > max_abs {
                        max_abs = v;
                    }
                }
            }

            if max_abs > 0.0 {
                let cap_linear = 10.0_f32.powf(self.safety_cap_db / 20.0);
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

            // Interpolate safety scale across the block to prevent zipper noise
            let t = i as f32 / self.fft_size as f32;
            let current_safety_scale = start_scale + t * (end_scale - start_scale);
            let final_scale = combined_scale * current_safety_scale;

            for ch in 0..self.num_output_channels {
                let sample = self.time_out_channels[ch][i] * final_scale;
                output[idx + ch] = sample;
            }
        }

        flush_denormals_inplace(output);
    }
}
