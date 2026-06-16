// ============================================================================
// Psychoacoustic Masking
// ============================================================================
//
// Implements frequency masking based on the Bark scale spreading function.
// Bins where the noise is perceptually masked by nearby signal content
// are skipped during denoising (gain set to 1.0), preserving signal quality.
//
// Reference: Zwicker & Fastl, "Psychoacoustics: Facts and Models", 1999

use super::DenoiserPlugin;
use math_audio_dsp::fast_math::fast_log10;

/// Small constant to prevent log(0)
const EPSILON: f32 = 1e-10;

/// Maximum spreading range in Bark
const MAX_SPREAD_BARK: f32 = 4.0;

/// Spreading slope below masker (dB/Bark, negative direction)
const LOWER_SLOPE_DB_PER_BARK: f32 = -10.0;

/// Spreading slope above masker (dB/Bark, positive direction)
const UPPER_SLOPE_DB_PER_BARK: f32 = -25.0;

/// Masking offset: how much below the masker level noise becomes inaudible (dB)
const MASKING_OFFSET_DB: f32 = -15.0;

impl DenoiserPlugin {
    /// Convert frequency in Hz to Bark scale (Zwicker formula)
    #[inline]
    fn freq_to_bark(freq_hz: f32) -> f32 {
        // Traunmüller (1990) approximation
        let f = freq_hz / 1000.0;
        13.0 * (0.76 * f).atan() + 3.5 * (f / 7.5).powi(2).atan()
    }

    /// Precompute Bark mapping and per-bin spreading ranges for all FFT bins.
    /// Called during initialize() when sample_rate is known.
    pub(super) fn precompute_bark_mapping(&mut self) {
        let bin_hz = self.config.sample_rate as f32 / self.config.fft_size as f32;
        for k in 0..self.config.spectrum_size {
            let freq = k as f32 * bin_hz;
            self.masking.bark_map[k] = Self::freq_to_bark(freq);
        }

        // Precompute the (lo, hi) bin range within MAX_SPREAD_BARK for each bin.
        // bark_map is monotonically non-decreasing, so we can use partition_point.
        for j in 0..self.config.spectrum_size {
            let bark_j = self.masking.bark_map[j];
            let lo = self.masking.bark_map[..self.config.spectrum_size]
                .partition_point(|&b| b < bark_j - MAX_SPREAD_BARK);
            let hi = self.masking.bark_map[..self.config.spectrum_size]
                .partition_point(|&b| b <= bark_j + MAX_SPREAD_BARK);
            self.masking.bark_bin_range[j] = (lo, hi);
        }
    }

    /// Compute masking thresholds for a given channel.
    /// Uses signal power and the Bark-scale spreading function.
    /// O(N) per bin thanks to precomputed bark_bin_range.
    ///
    /// Thresholds are stored in dB domain to avoid expensive powf() conversion.
    /// Comparison in `is_noise_masked` also works in dB.
    pub(super) fn compute_masking_thresholds(&mut self, channel: usize) {
        let n = self.config.spectrum_size;

        // Compute signal power in dB using fast approximation (1 fast_log10 per bin)
        for k in 0..n {
            let power = self.get_power_at_bin(channel, k).max(EPSILON);
            self.masking.masking_signal_power[k] = 10.0 * fast_log10(power);
        }

        // Initialize threshold to very low value (dB)
        self.masking.masking_threshold[..n].fill(f32::NEG_INFINITY);

        // For each masker bin, spread its masking energy only to nearby bins
        for j in 0..n {
            let masker_db = self.masking.masking_signal_power[j] + MASKING_OFFSET_DB;
            let bark_j = self.masking.bark_map[j];
            let (lo, hi) = self.masking.bark_bin_range[j];

            for k in lo..hi {
                let bark_diff = self.masking.bark_map[k] - bark_j;

                // Compute spreading attenuation
                let spread_db = if bark_diff < 0.0 {
                    // Below masker: gentler slope
                    LOWER_SLOPE_DB_PER_BARK * bark_diff.abs()
                } else {
                    // Above masker: steeper slope
                    UPPER_SLOPE_DB_PER_BARK * bark_diff
                };

                let threshold_contribution = masker_db + spread_db;

                // Take the maximum (most dominant masker wins)
                if threshold_contribution > self.masking.masking_threshold[k] {
                    self.masking.masking_threshold[k] = threshold_contribution;
                }
            }
        }

        // Thresholds remain in dB — no powf() conversion needed.
        // is_noise_masked() compares noise power in dB against these thresholds.
    }

    /// Check if noise at a given bin is perceptually masked.
    /// Compares noise power (converted to dB) against the dB-domain threshold.
    #[inline]
    pub(super) fn is_noise_masked(&self, channel: usize, bin: usize) -> bool {
        let noise_power = self.get_noise_power(channel, bin);
        let noise_db = 10.0 * fast_log10(noise_power);
        noise_db < self.masking.masking_threshold[bin]
    }
}
