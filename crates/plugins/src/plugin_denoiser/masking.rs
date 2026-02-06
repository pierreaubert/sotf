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

    /// Precompute Bark mapping for all FFT bins
    /// Called during initialize() when sample_rate is known
    pub(super) fn precompute_bark_mapping(&mut self) {
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;
        for k in 0..self.spectrum_size {
            let freq = k as f32 * bin_hz;
            self.bark_map[k] = Self::freq_to_bark(freq);
        }
    }

    /// Compute masking thresholds for a given channel
    /// Uses signal power and the Bark-scale spreading function
    pub(super) fn compute_masking_thresholds(&mut self, channel: usize) {
        let n = self.spectrum_size;

        // Copy signal power to scratch buffer
        for k in 0..n {
            self.masking_signal_power[k] = self.get_power_at_bin(channel, k).max(EPSILON);
        }

        // Convert to dB for spreading computation
        // and initialize threshold to very low value
        self.masking_threshold[..n].fill(f32::NEG_INFINITY);

        // For each masker bin, spread its masking energy to nearby bins
        for j in 0..n {
            let masker_db = 10.0 * self.masking_signal_power[j].log10() + MASKING_OFFSET_DB;
            let bark_j = self.bark_map[j];

            // Only spread to bins within MAX_SPREAD_BARK range
            for k in 0..n {
                let bark_k = self.bark_map[k];
                let bark_diff = bark_k - bark_j;

                if bark_diff.abs() > MAX_SPREAD_BARK {
                    continue;
                }

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
                if threshold_contribution > self.masking_threshold[k] {
                    self.masking_threshold[k] = threshold_contribution;
                }
            }
        }

        // Convert thresholds back from dB to linear power
        for k in 0..n {
            self.masking_threshold[k] = 10.0_f32.powf(self.masking_threshold[k] / 10.0);
        }
    }

    /// Check if noise at a given bin is perceptually masked
    #[inline]
    pub(super) fn is_noise_masked(&self, channel: usize, bin: usize) -> bool {
        let noise_power = self.get_noise_power(channel, bin);
        noise_power < self.masking_threshold[bin]
    }
}
