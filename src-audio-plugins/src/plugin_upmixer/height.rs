// ============================================================================
// Height Channel Processing
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Smooth height_band_gains to reduce bin-to-bin and frame-to-frame variance
    ///
    /// This applies:
    /// 1. Spectral smoothing: 3-point moving average across adjacent bins
    /// 2. Temporal smoothing: exponential averaging with previous frame
    ///
    /// This reduces "grainy" artifacts from bin-level processing within ERB bands.
    #[inline]
    pub(super) fn smooth_height_gains(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;

        // Temporal smoothing coefficient (higher = more smoothing)
        // 0.3 provides good balance: responsive but reduces frame-to-frame variance
        let temporal_alpha = 0.3_f32;

        // Spectral smoothing window size (3-point moving average)
        // Larger windows over-blur and lose frequency resolution
        let window_radius = 1_usize;

        // Temporary buffer for spectral smoothing result
        let mut smoothed = vec![0.0_f32; spectrum_size];

        // 1. Spectral smoothing: moving average across adjacent bins
        for i in 0..spectrum_size {
            let start = i.saturating_sub(window_radius);
            let end = (i + window_radius + 1).min(spectrum_size);

            let mut sum = 0.0_f32;
            let mut count = 0_usize;

            for j in start..end {
                sum += self.height_band_gains[j];
                count += 1;
            }

            smoothed[i] = if count > 0 {
                sum / count as f32
            } else {
                self.height_band_gains[i]
            };
        }

        // 2. Temporal smoothing: blend with previous frame
        for i in 0..spectrum_size {
            let current = smoothed[i];
            let previous = self.height_band_gains_prev[i];

            // Exponential moving average
            let blended = temporal_alpha * current + (1.0 - temporal_alpha) * previous;

            self.height_band_gains[i] = blended;
            self.height_band_gains_prev[i] = blended;
        }
    }
}
