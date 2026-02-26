// ============================================================================
// Height Channel Processing
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Smooth height_band_gains to reduce bin-to-bin and frame-to-frame variance
    ///
    /// This applies:
    /// 1. Spectral smoothing: 5-point sliding window average across adjacent bins
    ///    Edge bins handled separately so the main loop has a fixed 5-point window
    ///    with constant multiplier (* 0.2), enabling LLVM auto-vectorization.
    /// 2. Temporal smoothing: exponential averaging with previous frame
    ///
    /// This reduces "grainy" artifacts from bin-level processing within ERB bands.
    #[inline]
    pub(super) fn smooth_height_gains(&mut self) {
        let n = self.fft_size / 2 + 1;
        let mut smoothed = std::mem::take(&mut self.height_band_gains_temp);

        // Spectral smoothing: edge-separated 5-point moving average.
        // Handling edge bins separately eliminates per-iteration count tracking
        // and division, letting LLVM auto-vectorize the main loop (1021+ iterations).
        match n {
            0 => {}
            1 => {
                smoothed[0] = self.height_band_gains[0];
            }
            2 => {
                let s = (self.height_band_gains[0] + self.height_band_gains[1]) * 0.5;
                smoothed[0] = s;
                smoothed[1] = s;
            }
            3 => {
                let s = (self.height_band_gains[0]
                    + self.height_band_gains[1]
                    + self.height_band_gains[2])
                    / 3.0;
                smoothed[0] = s;
                smoothed[1] = s;
                smoothed[2] = s;
            }
            4 => {
                smoothed[0] = (self.height_band_gains[0]
                    + self.height_band_gains[1]
                    + self.height_band_gains[2])
                    / 3.0;
                let mid = (self.height_band_gains[0]
                    + self.height_band_gains[1]
                    + self.height_band_gains[2]
                    + self.height_band_gains[3])
                    * 0.25;
                smoothed[1] = mid;
                smoothed[2] = mid;
                smoothed[3] = (self.height_band_gains[1]
                    + self.height_band_gains[2]
                    + self.height_band_gains[3])
                    / 3.0;
            }
            _ => {
                let src = &self.height_band_gains[..n];

                // First 2 bins: partial window
                smoothed[0] = (src[0] + src[1] + src[2]) / 3.0;
                smoothed[1] = (src[0] + src[1] + src[2] + src[3]) * 0.25;

                // Main loop: fixed 5-point average, branchless, auto-vectorizable
                for i in 2..n - 2 {
                    smoothed[i] =
                        (src[i - 2] + src[i - 1] + src[i] + src[i + 1] + src[i + 2]) * 0.2;
                }

                // Last 2 bins: partial window
                smoothed[n - 2] = (src[n - 4] + src[n - 3] + src[n - 2] + src[n - 1]) * 0.25;
                smoothed[n - 1] = (src[n - 3] + src[n - 2] + src[n - 1]) / 3.0;
            }
        }

        // Temporal smoothing: asymmetric attack/release blend with previous frame.
        // Fast attack for transient ducking, slow release to prevent crackle on mask recovery.
        let attack_alpha = 0.25_f32;
        let release_alpha = 0.08_f32;
        for (s, (gain, prev)) in smoothed
            .iter()
            .zip(
                self.height_band_gains
                    .iter_mut()
                    .zip(self.height_band_gains_prev.iter_mut()),
            )
            .take(n)
        {
            let alpha = if *s < *prev {
                attack_alpha
            } else {
                release_alpha
            };
            let blended = alpha * s + (1.0 - alpha) * *prev;
            *gain = blended;
            *prev = blended;
        }

        self.height_band_gains_temp = smoothed;
    }
}
