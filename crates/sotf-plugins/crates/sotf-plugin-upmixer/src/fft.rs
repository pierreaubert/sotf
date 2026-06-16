// ============================================================================
// FFT Operations
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Phase 1: Apply window to input and perform forward FFT
    #[inline]
    pub(super) fn apply_window_and_forward_fft(&mut self, input: &[f32]) {
        // Deinterleave stereo input to L/R buffers using SIMD
        sotf_host::simd::deinterleave_stereo(
            input,
            &mut self.main_buffers.time_domain_left,
            &mut self.main_buffers.time_domain_right,
        );

        // Apply sqrt-Hann analysis window and headroom scale using SIMD.
        // The matching synthesis window is applied after inverse FFT.
        // Apply -3dB attenuation (1/sqrt(2)) to provide headroom for hot mixes
        let headroom_scale = std::f32::consts::FRAC_1_SQRT_2;
        sotf_host::simd::window_mul_simd_inplace(
            &mut self.main_buffers.time_domain_left,
            &self.main_buffers.window,
        );
        sotf_host::simd::window_mul_simd_inplace(
            &mut self.main_buffers.time_domain_right,
            &self.main_buffers.window,
        );

        if headroom_scale != 1.0 {
            sotf_host::simd::scale_add_simd_inplace(
                &mut self.main_buffers.time_domain_left,
                headroom_scale,
            );
            sotf_host::simd::scale_add_simd_inplace(
                &mut self.main_buffers.time_domain_right,
                headroom_scale,
            );
        }

        // Forward FFT (Real->Complex)
        self.fft
            .fft_forward
            .process(
                &mut self.main_buffers.time_domain_left,
                &mut self.main_buffers.freq_domain_left,
            )
            .unwrap();
        self.fft
            .fft_forward
            .process(
                &mut self.main_buffers.time_domain_right,
                &mut self.main_buffers.freq_domain_right,
            )
            .unwrap();
    }
}
