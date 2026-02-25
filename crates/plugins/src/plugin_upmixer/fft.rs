// ============================================================================
// FFT Operations
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Phase 1: Apply window to input and perform forward FFT
    #[inline]
    pub(super) fn apply_window_and_forward_fft(&mut self, input: &[f32]) {
        // Deinterleave stereo input to L/R buffers using SIMD
        crate::simd::deinterleave_stereo(
            input,
            &mut self.time_domain_left,
            &mut self.time_domain_right,
        );

        // Apply window and headroom scale using SIMD
        // Apply -3dB attenuation (1/sqrt(2)) to provide headroom for hot mixes
        let headroom_scale = std::f32::consts::FRAC_1_SQRT_2;
        crate::simd::window_mul_simd_inplace(&mut self.time_domain_left, &self.window);
        crate::simd::window_mul_simd_inplace(&mut self.time_domain_right, &self.window);

        if headroom_scale != 1.0 {
            crate::simd::scale_add_simd_inplace(&mut self.time_domain_left, headroom_scale);
            crate::simd::scale_add_simd_inplace(&mut self.time_domain_right, headroom_scale);
        }

        // Forward FFT (Real->Complex)
        self.fft_forward
            .process(&mut self.time_domain_left, &mut self.freq_domain_left)
            .unwrap();
        self.fft_forward
            .process(&mut self.time_domain_right, &mut self.freq_domain_right)
            .unwrap();
    }
}
