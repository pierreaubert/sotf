// ============================================================================
// Output Processing
// ============================================================================

use super::UpmixerPlugin;

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
        let mut safety_scale = 1.0_f32;
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
                if effective_peak > cap_linear {
                    safety_scale = cap_linear / effective_peak;
                }
            }
        }

        let final_scale = combined_scale * safety_scale;

        for i in 0..self.fft_size {
            let idx = i * self.num_output_channels;
            for ch in 0..self.num_output_channels {
                let mut sample = self.time_out_channels[ch][i] * final_scale;

                // Flush denormals to zero to prevent CPU spikes and audio glitches
                // Denormal numbers (very small floats near zero) can cause significant
                // performance degradation and numerical instability
                if sample.abs() < 1e-30 {
                    sample = 0.0;
                }

                output[idx + ch] = sample;
            }
        }
    }
}
