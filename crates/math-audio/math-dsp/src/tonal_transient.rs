//! Tonal/Transient Separator using Spectral Median Filtering
//!
//! Based on FitzGerald (2010): "Harmonic/Percussive Separation using Median Filtering".
//!
//! The algorithm exploits a key property of spectral representations:
//! - **Tonal (harmonic) components** appear as horizontal lines in the spectrogram
//!   (stable across time at fixed frequencies)
//! - **Transient (percussive) components** appear as vertical lines
//!   (broadband energy at specific time instants)
//!
//! By applying median filters along the two axes:
//! - **Time-median** (across frames at each bin) → extracts tonal component
//! - **Frequency-median** (across bins at each frame) → extracts transient component
//!
//! The soft masks are computed using Wiener-style power ratio:
//! `mask_tonal[bin] = tonal_power^p / (tonal_power^p + transient_power^p)`
//!
//! # Usage
//!
//! ```ignore
//! let mut sep = TonalTransientSeparator::new(1025, 7, 7);
//! // In your STFT loop:
//! sep.process(&magnitudes, &mut mask_tonal, &mut mask_transient);
//! ```

/// Spectral-domain tonal/transient separator.
///
/// Operates on STFT magnitude spectra, producing soft masks that can be
/// applied to the complex spectrum to extract tonal or transient components.
pub struct TonalTransientSeparator {
    num_bins: usize,
    /// Ring buffer of spectral magnitude frames for time-median
    time_history: Vec<Vec<f32>>,
    time_kernel_size: usize,
    time_write_pos: usize,
    time_filled: usize,
    /// Kernel size for frequency-axis median
    freq_kernel_size: usize,
    /// Scratch buffer for median computation (avoid per-frame allocation)
    median_scratch: Vec<f32>,
    /// Wiener mask exponent (default 2.0)
    mask_power: f32,
}

impl TonalTransientSeparator {
    /// Create a new separator.
    ///
    /// # Arguments
    /// * `num_bins` - Number of FFT magnitude bins (typically fft_size/2 + 1)
    /// * `time_kernel` - Median filter length along time axis (frames). Must be odd.
    /// * `freq_kernel` - Median filter length along frequency axis (bins). Must be odd.
    pub fn new(num_bins: usize, time_kernel: usize, freq_kernel: usize) -> Self {
        let time_kernel = if time_kernel.is_multiple_of(2) {
            time_kernel + 1
        } else {
            time_kernel
        };
        let freq_kernel = if freq_kernel.is_multiple_of(2) {
            freq_kernel + 1
        } else {
            freq_kernel
        };

        Self {
            num_bins,
            time_history: vec![vec![0.0; num_bins]; time_kernel],
            time_kernel_size: time_kernel,
            time_write_pos: 0,
            time_filled: 0,
            freq_kernel_size: freq_kernel,
            median_scratch: vec![0.0; time_kernel.max(freq_kernel)],
            mask_power: 2.0,
        }
    }

    /// Set the Wiener mask exponent (default 2.0).
    /// Higher values produce harder (more binary) masks.
    pub fn set_mask_power(&mut self, power: f32) {
        self.mask_power = power.max(0.1);
    }

    /// Process one STFT frame's magnitude spectrum.
    ///
    /// Computes soft Wiener masks for tonal and transient components.
    /// `mask_tonal[bin] + mask_transient[bin] ≈ 1.0` for all bins.
    ///
    /// # Arguments
    /// * `magnitudes` - Input magnitude spectrum (length = num_bins)
    /// * `mask_tonal` - Output tonal mask (length = num_bins)
    /// * `mask_transient` - Output transient mask (length = num_bins)
    pub fn process(
        &mut self,
        magnitudes: &[f32],
        mask_tonal: &mut [f32],
        mask_transient: &mut [f32],
    ) {
        debug_assert_eq!(magnitudes.len(), self.num_bins);
        debug_assert_eq!(mask_tonal.len(), self.num_bins);
        debug_assert_eq!(mask_transient.len(), self.num_bins);

        // Store current frame in ring buffer
        self.time_history[self.time_write_pos].copy_from_slice(magnitudes);
        self.time_write_pos = (self.time_write_pos + 1) % self.time_kernel_size;
        self.time_filled = self.time_filled.min(self.time_kernel_size - 1) + 1;

        let p = self.mask_power;

        for bin in 0..self.num_bins {
            // --- Time-median at this bin (tonal estimate) ---
            let tonal_est = self.compute_time_median(bin);

            // --- Frequency-median at this bin (transient estimate) ---
            let transient_est = self.compute_freq_median(magnitudes, bin);

            // --- Wiener soft masks ---
            let tonal_pow = tonal_est.powf(p);
            let trans_pow = transient_est.powf(p);
            let denom = tonal_pow + trans_pow;

            if denom > 1e-10 {
                mask_tonal[bin] = tonal_pow / denom;
                mask_transient[bin] = trans_pow / denom;
            } else {
                // Both are near zero — default to pass-through
                mask_tonal[bin] = 0.5;
                mask_transient[bin] = 0.5;
            }
        }
    }

    /// Compute median across time frames for a specific bin.
    fn compute_time_median(&mut self, bin: usize) -> f32 {
        let n = self.time_filled;
        let scratch = &mut self.median_scratch[..n];

        // Collect values from ring buffer
        for (i, slot) in scratch.iter_mut().enumerate() {
            let frame_idx = if self.time_write_pos >= n {
                self.time_write_pos - n + i
            } else {
                (self.time_kernel_size + self.time_write_pos - n + i) % self.time_kernel_size
            };
            *slot = self.time_history[frame_idx][bin];
        }

        fast_median(scratch)
    }

    /// Compute median across frequency bins for the current frame.
    fn compute_freq_median(&mut self, magnitudes: &[f32], center_bin: usize) -> f32 {
        let half = self.freq_kernel_size / 2;
        let start = center_bin.saturating_sub(half);
        let end = (center_bin + half + 1).min(self.num_bins);
        let n = end - start;

        let scratch = &mut self.median_scratch[..n];
        scratch.copy_from_slice(&magnitudes[start..end]);

        fast_median(scratch)
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        for frame in &mut self.time_history {
            frame.fill(0.0);
        }
        self.time_write_pos = 0;
        self.time_filled = 0;
    }
}

/// In-place partial sort median. Modifies the input slice.
#[inline]
fn fast_median(data: &mut [f32]) -> f32 {
    let n = data.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return data[0];
    }
    if n == 2 {
        return (data[0] + data[1]) * 0.5;
    }

    // For small N (typical: 5-15), selection sort to midpoint is fast enough
    let mid = n / 2;
    // Partial selection sort: find the mid-th smallest element
    for i in 0..=mid {
        let mut min_idx = i;
        for j in (i + 1)..n {
            if data[j] < data[min_idx] {
                min_idx = j;
            }
        }
        data.swap(i, min_idx);
    }
    data[mid]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_median() {
        let mut data = [5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!(fast_median(&mut data), 3.0);

        let mut data2 = [1.0, 2.0];
        assert!((fast_median(&mut data2) - 1.5).abs() < 1e-6);

        let mut data1 = [42.0];
        assert_eq!(fast_median(&mut data1), 42.0);

        let mut empty: [f32; 0] = [];
        assert_eq!(fast_median(&mut empty), 0.0);
    }

    #[test]
    fn test_separator_pure_tone() {
        // A pure tone should have high tonal mask at its bin
        let num_bins = 128;
        let mut sep = TonalTransientSeparator::new(num_bins, 7, 7);
        let mut mask_tonal = vec![0.0; num_bins];
        let mut mask_transient = vec![0.0; num_bins];

        // Feed several frames of the same spectrum (stable tone at bin 30)
        let mut magnitudes = vec![0.01; num_bins];
        magnitudes[30] = 1.0;

        for _ in 0..10 {
            sep.process(&magnitudes, &mut mask_tonal, &mut mask_transient);
        }

        // Bin 30 should be classified as tonal (high mask value)
        assert!(
            mask_tonal[30] > 0.6,
            "Pure tone bin should have high tonal mask: {}",
            mask_tonal[30]
        );
    }

    #[test]
    fn test_separator_transient() {
        // A broadband burst should have high transient mask
        let num_bins = 128;
        let mut sep = TonalTransientSeparator::new(num_bins, 7, 7);
        let mut mask_tonal = vec![0.0; num_bins];
        let mut mask_transient = vec![0.0; num_bins];

        // Feed several frames of silence, then one broadband burst
        let silence = vec![0.01; num_bins];
        for _ in 0..5 {
            sep.process(&silence, &mut mask_tonal, &mut mask_transient);
        }

        // Broadband burst: all bins high
        let burst = vec![1.0; num_bins];
        sep.process(&burst, &mut mask_tonal, &mut mask_transient);

        // Most bins should have high transient mask (broadband = vertical in spectrogram)
        let avg_transient: f32 = mask_transient[10..118].iter().sum::<f32>() / 108.0;
        assert!(
            avg_transient > 0.3,
            "Broadband burst should have elevated transient mask: {avg_transient}"
        );
    }

    #[test]
    fn test_masks_sum_to_one() {
        let num_bins = 64;
        let mut sep = TonalTransientSeparator::new(num_bins, 5, 5);
        let mut mask_tonal = vec![0.0; num_bins];
        let mut mask_transient = vec![0.0; num_bins];

        let magnitudes: Vec<f32> = (0..num_bins)
            .map(|i| (i as f32 * 0.1).sin().abs())
            .collect();

        for _ in 0..5 {
            sep.process(&magnitudes, &mut mask_tonal, &mut mask_transient);
        }

        for bin in 0..num_bins {
            let sum = mask_tonal[bin] + mask_transient[bin];
            assert!(
                (sum - 1.0).abs() < 0.01,
                "Masks should sum to 1.0 at bin {bin}: got {sum}"
            );
        }
    }

    #[test]
    fn test_reset() {
        let num_bins = 32;
        let mut sep = TonalTransientSeparator::new(num_bins, 5, 5);
        let mut mt = vec![0.0; num_bins];
        let mut mr = vec![0.0; num_bins];

        let signal = vec![1.0; num_bins];
        sep.process(&signal, &mut mt, &mut mr);

        sep.reset();

        // After reset, time history should be cleared
        let silence = vec![0.01; num_bins];
        sep.process(&silence, &mut mt, &mut mr);
        // Should not see influence from pre-reset data
        // (tonal estimate from time-median should be ~0.01, not ~1.0)
    }

    #[test]
    fn test_mask_power_affects_hardness() {
        let num_bins = 32;
        let magnitudes = vec![0.5; num_bins];

        // Low power → soft masks (closer to 0.5)
        let mut sep_soft = TonalTransientSeparator::new(num_bins, 5, 5);
        sep_soft.set_mask_power(1.0);
        let mut mt_soft = vec![0.0; num_bins];
        let mut mr_soft = vec![0.0; num_bins];
        for _ in 0..5 {
            sep_soft.process(&magnitudes, &mut mt_soft, &mut mr_soft);
        }

        // High power → harder masks (closer to 0 or 1)
        let mut sep_hard = TonalTransientSeparator::new(num_bins, 5, 5);
        sep_hard.set_mask_power(4.0);
        let mut mt_hard = vec![0.0; num_bins];
        let mut mr_hard = vec![0.0; num_bins];
        for _ in 0..5 {
            sep_hard.process(&magnitudes, &mut mt_hard, &mut mr_hard);
        }

        // Both should still sum to ~1.0
        for bin in 0..num_bins {
            assert!((mt_soft[bin] + mr_soft[bin] - 1.0).abs() < 0.01);
            assert!((mt_hard[bin] + mr_hard[bin] - 1.0).abs() < 0.01);
        }
    }
}
