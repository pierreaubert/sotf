// ============================================================================
// FFT Operations
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Phase 1: Apply window to input and perform forward FFT
    #[inline]
    pub(super) fn apply_window_and_forward_fft(&mut self, input: &[f32]) {
        // Copy input to time domain buffers and apply ANALYSIS window
        for i in 0..self.fft_size {
            let idx = i * 2;
            let window_val = self.window[i];
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
