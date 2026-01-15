// ============================================================================
// FFT Operations
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Phase 1: Apply window to input and perform forward FFT
    #[inline]
    pub(super) fn apply_window_and_forward_fft(&mut self, input: &[f32]) {
        // Copy input to time domain buffers and apply ANALYSIS window
        // Apply -3dB attenuation (0.707) to provide headroom for hot mixes
        let headroom_scale = 0.70710678;
        for i in 0..self.fft_size {
            let idx = i * 2;
            let window_val = self.window[i] * headroom_scale;
            self.time_domain_left[i] = input[idx] * window_val;
            self.time_domain_right[i] = input[idx + 1] * window_val;
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
