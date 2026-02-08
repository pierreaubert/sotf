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

        // Asymmetric temporal smoothing: fast attack for transient ducking,
        // slow release to prevent crackle on mask recovery
        let attack_alpha = 0.25_f32;
        let release_alpha = 0.08_f32;

        // Spectral smoothing window size (5-point moving average)
        // Wider window smooths ERB-band staircase edges
        let window_radius = 2_usize;

        // Use pre-allocated temporary buffer for spectral smoothing result
        let mut smoothed = std::mem::take(&mut self.height_band_gains_temp);

        // 1. Spectral smoothing: moving average across adjacent bins
        for (i, smoothed_val) in smoothed.iter_mut().enumerate().take(spectrum_size) {
            let start = i.saturating_sub(window_radius);
            let end = (i + window_radius + 1).min(spectrum_size);

            let mut sum = 0.0_f32;
            let mut count = 0_usize;

            for j in start..end {
                sum += self.height_band_gains[j];
                count += 1;
            }

            *smoothed_val = if count > 0 {
                sum / count as f32
            } else {
                self.height_band_gains[i]
            };
        }

        // 2. Temporal smoothing: asymmetric attack/release blend with previous frame
        for (i, current) in smoothed.iter().enumerate().take(spectrum_size) {
            let previous = self.height_band_gains_prev[i];

            // Use fast attack when mask decreases (ducking), slow release when recovering
            let alpha = if *current < previous {
                attack_alpha
            } else {
                release_alpha
            };
            let blended = alpha * current + (1.0 - alpha) * previous;

            self.height_band_gains[i] = blended;
            self.height_band_gains_prev[i] = blended;
        }

        // Restore pre-allocated buffer
        self.height_band_gains_temp = smoothed;
    }
}
