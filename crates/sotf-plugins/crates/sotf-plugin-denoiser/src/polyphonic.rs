// ============================================================================
// Polyphonic Note Detection Gain Calculation
// ============================================================================
//
// This module implements a "Soft Spectral Gate" approach focused on preserving
// tonal content (notes) while smoothly attenuating background.
//
// It uses PND (Polyphonic Note Detection) to identify tonal peaks, then applies
// a cosine taper around each peak for smooth transitions between unity gain and
// floor gain. This avoids the binary pass/floor artifacts of a hard gate.

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

/// Half-width of the cosine taper in bins around each detected peak.
/// This defines the smooth transition zone: bins within TAPER_HALF_WIDTH
/// of a peak center get a cosine-tapered gain between floor and 1.0.
const TAPER_HALF_WIDTH: usize = 4;

impl DenoiserPlugin {
    /// Calculate and apply Polyphonic Note Detection (Soft Spectral Gate) gains
    ///
    /// Three-pass approach:
    /// 1. Compute cosine-tapered gate gains per bin based on detected peaks
    /// 2. Smooth gains across frequency bins (prevents musical noise)
    /// 3. Apply temporal smoothing with attack/release envelope
    pub(super) fn calculate_polyphonic_gains(&mut self) {
        let floor_linear = self.floor_linear;
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;

        let mut total_reduction = 0.0_f32;
        let mut bin_count = 0;

        for ch in 0..self.channels {
            // Pass 1: Initialize all bins to floor gain
            for k in 0..self.spectrum_size {
                self.gain[ch][k] = floor_linear;
            }

            // Apply cosine-tapered gain around detected tonal peaks.
            // For each peak, the center bin(s) get unity gain, and surrounding
            // bins within the taper zone get a smooth cosine blend from 1.0
            // down to floor_linear.
            let peaks = self.pnd_analyzers[ch].current_matched_peaks();
            for &(freq_hz, _mag) in peaks {
                let center_k = (freq_hz / bin_hz).round() as usize;

                // Apply cosine taper around the peak center
                let taper_start = center_k.saturating_sub(TAPER_HALF_WIDTH);
                let taper_end = (center_k + TAPER_HALF_WIDTH).min(self.spectrum_size - 1);

                for k in taper_start..=taper_end {
                    let dist = (k as f32 - center_k as f32).abs();
                    let taper_gain = if dist <= 1.0 {
                        // Center bin and immediate neighbors: unity gain
                        1.0
                    } else {
                        // Cosine taper from 1.0 to 0.0 over TAPER_HALF_WIDTH bins
                        let t = (dist - 1.0) / (TAPER_HALF_WIDTH as f32 - 1.0);
                        let cosine_weight = 0.5 * (1.0 + (std::f32::consts::PI * t.min(1.0)).cos());
                        // Blend between floor and 1.0
                        floor_linear + cosine_weight * (1.0 - floor_linear)
                    };
                    // Take the max in case of overlapping tapers from adjacent peaks
                    self.gain[ch][k] = self.gain[ch][k].max(taper_gain);
                }
            }

            // Pass 2: Smooth gains across frequency bins
            self.smooth_gains_across_frequency(ch);

            // Pass 2b: Psychoacoustic masking — skip denoising for masked bins
            if self.psychoacoustic_masking {
                self.compute_masking_thresholds(ch);
                for k in 0..self.spectrum_size {
                    if self.is_noise_masked(ch, k) {
                        self.gain[ch][k] = 1.0;
                    }
                }
            }

            // Pass 3: Apply temporal smoothing with attack/release
            for k in 0..self.spectrum_size {
                let target_gain = self.gain[ch][k];
                let prev_gain = self.smoothed_gain[ch][k];
                let coeff = if target_gain > prev_gain {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };
                let smoothed = target_gain + coeff * (prev_gain - target_gain);
                self.smoothed_gain[ch][k] = smoothed;

                total_reduction += (1.0 - smoothed).max(0.0);
                bin_count += 1;
            }
        }

        // Update average reduction in dB for monitoring
        if bin_count > 0 {
            let avg_gain = 1.0 - (total_reduction / bin_count as f32);
            self.avg_reduction_db = if avg_gain > EPSILON {
                -20.0 * avg_gain.log10()
            } else {
                60.0
            };
        }
    }
}
