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

use math_audio_dsp::fast_math::fast_log10;

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
        let reduction_factor = self.reduction_linear;
        let floor_linear = self.floor_linear;
        let transparency = self.transparency;

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
                    let dd_term = prev_gain * prev_gain * prev_pow / noise_power.max(EPSILON);
                    dd_alpha * dd_term + (1.0 - dd_alpha) * ml_term
                } else {
                    ((signal_power - noise_power).max(0.0)) / noise_power.max(EPSILON)
                };

                // Store current power for next frame's DD computation
                self.prev_power[ch][k] = signal_power;

                // Calculate Wiener gain with reduction control in denominator
                // This preserves gain in high-SNR (clean) regions while reducing
                // gain in low-SNR (noisy) regions proportional to reduction_factor
                let gain = (snr_priori / (snr_priori + reduction_factor)).max(floor_linear);

                // Blend toward dry signal based on transparency (0 = full denoise, 1 = pass-through)
                let gain = gain + transparency * (1.0 - gain);
                self.gain[ch][k] = gain;
            }

            // Pass 1b: Spectral subtraction (combine with Wiener via min)
            if self.spectral_sub_enabled {
                self.calculate_spectral_subtraction_gains_for_channel(ch);
            }

            // Pass 1c: Hiss removal (additional high-frequency attenuation)
            if self.hiss_enabled {
                self.apply_hiss_removal(ch);
            }

            // Pass 2: Smooth gains across frequency bins
            if self.spectral_smoothing_enabled {
                self.smooth_gains_across_frequency(ch);
            }

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
            if self.temporal_smoothing_enabled {
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
            } else {
                for k in 0..self.spectrum_size {
                    self.smoothed_gain[ch][k] = self.gain[ch][k];
                    total_reduction += (1.0 - self.gain[ch][k]).max(0.0);
                    bin_count += 1;
                }
            }
        }

        // Update average reduction in dB for monitoring
        if bin_count > 0 {
            let avg_gain = 1.0 - (total_reduction / bin_count as f32);
            self.avg_reduction_db = if avg_gain > EPSILON {
                -20.0 * fast_log10(avg_gain)
            } else {
                60.0 // Max reduction
            };
        }
    }

    /// Compute the 5-tap Gaussian kernel weights for a given smoothing value.
    /// Returns (c0, c1, c2) normalized weights. Returns (1, 0, 0) if smoothing is zero.
    pub(super) fn compute_smoothing_kernel(smoothing: f32) -> (f32, f32, f32) {
        if smoothing < EPSILON {
            return (1.0, 0.0, 0.0);
        }
        let sigma = smoothing * 2.0;
        let inv_2sigma_sq = 0.5 / (sigma * sigma);
        let w0 = 1.0_f32;
        let w1 = (-inv_2sigma_sq).exp();
        let w2 = (-4.0 * inv_2sigma_sq).exp();
        let sum = w0 + 2.0 * w1 + 2.0 * w2;
        (w0 / sum, w1 / sum, w2 / sum)
    }

    /// Smooth gains across frequency bins using a 5-tap Gaussian kernel.
    /// The `smoothing` parameter controls the kernel width (σ = smoothing × 2 bins).
    /// Uses replicate boundary conditions at edges.
    pub(super) fn smooth_gains_across_frequency(&mut self, channel: usize) {
        let (c0, c1, c2) = self.freq_smooth_kernel;
        if c1 == 0.0 && c2 == 0.0 {
            return; // No smoothing needed
        }

        let n = self.spectrum_size;

        // Copy current gains to scratch buffer
        self.freq_smooth_temp[..n].copy_from_slice(&self.gain[channel][..n]);

        let src = &self.freq_smooth_temp;
        let dst = &mut self.gain[channel];

        // Apply 5-tap Gaussian kernel split into three segments to enable autovectorization.
        // The main body (k=2..n-2) uses direct index arithmetic with no boundary checks,
        // allowing the compiler to vectorize the inner loop. Edges use replicate conditions.

        // Edge-left: k = 0..2 (boundary: clamp to 0)
        for k in 0..2.min(n) {
            let km2 = k.saturating_sub(2);
            let km1 = k.saturating_sub(1);
            let kp1 = (k + 1).min(n - 1);
            let kp2 = (k + 2).min(n - 1);
            dst[k] = c2 * src[km2] + c1 * src[km1] + c0 * src[k] + c1 * src[kp1] + c2 * src[kp2];
        }

        // Main body: k = 2..n-2 (no boundary checks — compiler can autovectorize)
        if n > 4 {
            for k in 2..n - 2 {
                dst[k] = c2 * src[k - 2]
                    + c1 * src[k - 1]
                    + c0 * src[k]
                    + c1 * src[k + 1]
                    + c2 * src[k + 2];
            }
        }

        // Edge-right: k = n-2..n (boundary: clamp to n-1)
        for k in (n - 2).max(2)..n {
            let km2 = k.saturating_sub(2);
            let km1 = k.saturating_sub(1);
            let kp1 = (k + 1).min(n - 1);
            let kp2 = (k + 2).min(n - 1);
            dst[k] = c2 * src[km2] + c1 * src[km1] + c0 * src[k] + c1 * src[kp1] + c2 * src[kp2];
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
        self.reduction_linear = 10.0_f32.powf(self.reduction_db / 10.0);
        self.floor_linear = 10.0_f32.powf(self.floor_db / 20.0);
    }

    /// Compute average estimated noise floor in dB (averaged across channels).
    /// Writes into `self.cached_noise_floor_buf` to avoid allocations.
    pub(super) fn compute_noise_floor_db(&mut self) {
        let num_display_bands = self.cached_noise_floor_buf.len();
        let bins_per_band = (self.spectrum_size / num_display_bands).max(1);

        for band in 0..num_display_bands {
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

            self.cached_noise_floor_buf[band] = if count > 0 {
                let avg_power = sum / count as f32;
                10.0 * fast_log10(avg_power)
            } else {
                -100.0
            };
        }
    }

    /// Compute current SNR estimate per band in dB (averaged across channels).
    /// Writes into `self.cached_snr_buf` to avoid allocations.
    pub(super) fn compute_snr_db(&mut self) {
        let num_display_bands = self.cached_snr_buf.len();
        let bins_per_band = (self.spectrum_size / num_display_bands).max(1);

        for band in 0..num_display_bands {
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

            self.cached_snr_buf[band] = if count > 0 {
                let avg_snr = sum_snr / count as f32;
                10.0 * fast_log10(avg_snr)
            } else {
                0.0
            };
        }
    }
}
