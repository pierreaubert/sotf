// ============================================================================
// Wiener Filter Gain Calculation
// ============================================================================
//
// The Wiener filter provides optimal noise reduction by estimating the
// signal-to-noise ratio (SNR) and applying frequency-dependent attenuation.
//
// Wiener gain formula: G(k) = SNR(k) / (SNR(k) + 1)
// Where SNR(k) = max(|X(k)|² - σ_n²(k), 0) / σ_n²(k)
//
// The gain is bounded by a floor to prevent musical noise artifacts.

use super::DenoiserPlugin;

/// Small constant to prevent division by zero
const EPSILON: f32 = 1e-10;

impl DenoiserPlugin {
    /// Calculate and apply Wiener filter gains for all channels
    ///
    /// This method:
    /// 1. Calculates SNR for each frequency bin
    /// 2. Computes Wiener gain with reduction control
    /// 3. Applies floor to prevent musical noise
    /// 4. Smooths gains with attack/release envelope
    pub(super) fn calculate_wiener_gains(&mut self) {
        let reduction_factor = 10.0_f32.powf(self.reduction_db / 10.0);
        let floor_linear = 10.0_f32.powf(self.floor_db / 20.0);

        let mut total_reduction = 0.0_f32;
        let mut bin_count = 0;

        for ch in 0..self.channels {
            for k in 0..self.spectrum_size {
                // Get signal and noise power
                let signal_power = self.get_power_at_bin(ch, k);
                let noise_power = self.get_noise_power(ch, k);

                // Calculate a priori SNR estimate
                let snr_priori = ((signal_power - noise_power).max(0.0)) / noise_power.max(EPSILON);

                // Apply reduction control
                // reduction_factor > 1 means more aggressive noise reduction
                let effective_snr = snr_priori / reduction_factor;

                // Calculate Wiener gain
                let mut gain = effective_snr / (effective_snr + 1.0);

                // Apply floor to prevent musical noise
                gain = gain.max(floor_linear);

                // Store instantaneous gain for smoothing
                self.gain[ch][k] = gain;

                // Apply temporal smoothing with attack/release
                let prev_gain = self.smoothed_gain[ch][k];
                let coeff = if gain > prev_gain {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };
                let smoothed = gain + coeff * (prev_gain - gain);
                self.smoothed_gain[ch][k] = smoothed;

                // Track average reduction for monitoring
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
                60.0 // Max reduction
            };
        }
    }

    /// Calculate time coefficient for envelope follower
    /// Converts time in milliseconds to exponential smoothing coefficient
    #[inline]
    pub(super) fn time_to_coeff(time_ms: f32, sample_rate: u32, hop_size: usize) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            // Adjust for hop-based frame rate, not sample rate
            let frame_rate = sample_rate as f32 / hop_size as f32;
            (-1.0 / (time_ms * 0.001 * frame_rate)).exp()
        }
    }

    /// Update attack/release coefficients when parameters change
    pub(super) fn update_envelope_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_ms, self.sample_rate, self.hop_size);
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate, self.hop_size);
        self.floor_linear = 10.0_f32.powf(self.floor_db / 20.0);
    }

    /// Get average estimated noise floor in dB (averaged across channels)
    pub(super) fn get_noise_floor_db(&self) -> Vec<f32> {
        // Downsample to ~30 bands for display
        let num_display_bands = 30;
        let bins_per_band = (self.spectrum_size / num_display_bands).max(1);

        let mut noise_floor = vec![0.0_f32; num_display_bands];

        for (band, noise_val) in noise_floor.iter_mut().enumerate() {
            let start_bin = band * bins_per_band;
            let end_bin = ((band + 1) * bins_per_band).min(self.spectrum_size);

            let mut sum = 0.0_f32;
            let mut count = 0;

            for k in start_bin..end_bin {
                for ch in 0..self.channels {
                    let power = self.noise_psd[ch][k].max(EPSILON);
                    sum += power;
                    count += 1;
                }
            }

            if count > 0 {
                let avg_power = sum / count as f32;
                *noise_val = 10.0 * avg_power.log10();
            } else {
                *noise_val = -100.0;
            }
        }

        noise_floor
    }

    /// Get current SNR estimate per band in dB (averaged across channels)
    pub(super) fn get_snr_db(&self) -> Vec<f32> {
        let num_display_bands = 30;
        let bins_per_band = (self.spectrum_size / num_display_bands).max(1);

        let mut snr = vec![0.0_f32; num_display_bands];

        for (band, snr_val) in snr.iter_mut().enumerate() {
            let start_bin = band * bins_per_band;
            let end_bin = ((band + 1) * bins_per_band).min(self.spectrum_size);

            let mut sum_snr = 0.0_f32;
            let mut count = 0;

            for k in start_bin..end_bin {
                for ch in 0..self.channels {
                    let signal_power = self.get_power_at_bin(ch, k).max(EPSILON);
                    let noise_power = self.noise_psd[ch][k].max(EPSILON);
                    let bin_snr = signal_power / noise_power;
                    sum_snr += bin_snr;
                    count += 1;
                }
            }

            if count > 0 {
                let avg_snr = sum_snr / count as f32;
                *snr_val = 10.0 * avg_snr.log10();
            } else {
                *snr_val = 0.0;
            }
        }

        snr
    }
}
