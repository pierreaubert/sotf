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
    /// Three-pass approach:
    /// 1. Compute instantaneous Wiener gain per bin
    /// 2. Smooth gains across frequency bins (prevents musical noise)
    /// 3. Apply temporal smoothing with attack/release envelope
    pub(super) fn calculate_wiener_gains(&mut self) {
        let reduction_factor = 10.0_f32.powf(self.reduction_db / 10.0);
        let floor_linear = 10.0_f32.powf(self.floor_db / 20.0);

        let mut total_reduction = 0.0_f32;
        let mut bin_count = 0;

        let dd_enabled = self.dd_enabled;
        let dd_alpha = self.dd_alpha;

        for ch in 0..self.channels {
            // Pass 1: Compute instantaneous Wiener gain
            for k in 0..self.spectrum_size {
                let signal_power = self.get_power_at_bin(ch, k);
                let noise_power = self.get_effective_noise_power(ch, k);

                // Calculate a priori SNR estimate
                let snr_priori = if dd_enabled {
                    // Decision-Directed (Ephraim-Malah) approach:
                    // SNR_dd = α * G²_prev * P_prev / σ_n² + (1-α) * max(P/σ_n² - 1, 0)
                    let prev_gain = self.smoothed_gain[ch][k];
                    let prev_pow = self.prev_power[ch][k];
                    let ml_term = (signal_power / noise_power.max(EPSILON) - 1.0).max(0.0);
                    let dd_term =
                        prev_gain * prev_gain * prev_pow / noise_power.max(EPSILON);
                    dd_alpha * dd_term + (1.0 - dd_alpha) * ml_term
                } else {
                    ((signal_power - noise_power).max(0.0)) / noise_power.max(EPSILON)
                };

                // Store current power for next frame's DD computation
                self.prev_power[ch][k] = signal_power;

                // Apply reduction control
                let effective_snr = snr_priori / reduction_factor;

                // Calculate Wiener gain with floor
                let gain = (effective_snr / (effective_snr + 1.0)).max(floor_linear);
                self.gain[ch][k] = gain;
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
                let gain = self.gain[ch][k];
                let prev_gain = self.smoothed_gain[ch][k];
                let coeff = if gain > prev_gain {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };
                let smoothed = gain + coeff * (prev_gain - gain);
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
                60.0 // Max reduction
            };
        }
    }

    /// Smooth gains across frequency bins using a 3-tap triangular kernel [β, 1-2β, β]
    /// where β = smoothing / 2. Uses freq_smooth_temp as scratch buffer.
    pub(super) fn smooth_gains_across_frequency(&mut self, channel: usize) {
        let beta = self.smoothing / 2.0;
        if beta < EPSILON {
            return; // No smoothing needed
        }

        let n = self.spectrum_size;
        // Copy current gains to scratch buffer
        self.freq_smooth_temp[..n].copy_from_slice(&self.gain[channel][..n]);

        // Apply 3-tap kernel: out[k] = β * in[k-1] + (1-2β) * in[k] + β * in[k+1]
        let center = 1.0 - 2.0 * beta;
        let src = &self.freq_smooth_temp;
        let dst = &mut self.gain[channel];

        // First bin: no left neighbor
        dst[0] = center * src[0] + beta * src[1];
        // Normalize: center + beta = 1-beta, so divide by (1-beta)
        dst[0] /= 1.0 - beta;

        // Interior bins
        for k in 1..n - 1 {
            dst[k] = beta * src[k - 1] + center * src[k] + beta * src[k + 1];
        }

        // Last bin: no right neighbor
        if n > 1 {
            dst[n - 1] = beta * src[n - 2] + center * src[n - 1];
            dst[n - 1] /= 1.0 - beta;
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
