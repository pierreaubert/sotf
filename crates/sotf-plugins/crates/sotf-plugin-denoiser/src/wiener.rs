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

// ============================================================================
// Formant Preserver
// ============================================================================
//
// Preserves speech formant structure (spectral envelope peaks) during denoising
// by flooring the Wiener gain at formant peaks.
//
// The spectral envelope is estimated with a moving-average smoother applied to
// the log-magnitude spectrum — a computationally cheap approximation of the
// real cepstrum low-pass lifter (IFT → keep N coefficients → FT). The smoother
// window is `lifter_len * 2` bins wide, which corresponds to keeping roughly
// the first `lifter_len` cepstral coefficients and thus captures slowly-varying
// formant peaks while ignoring fine harmonic structure.
//
// At every bin where the smoothed envelope is more than 3 dB above the mean
// envelope level (i.e., a formant peak), the Wiener gain is floored to:
//   floor = formant_strength * 0.3
// The 0.3 factor means full-strength preservation still applies 30% gain
// (−10 dB) to formant peaks even under heavy noise reduction.

/// Number of cepstral coefficients to keep (controls envelope smoothness).
/// 30 coefficients is a classic choice that captures F1–F4 formants for speech.
const LIFTER_LEN: usize = 30;

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

            // Pass 1c: Formant preservation — floor gains at spectral envelope peaks
            // so that speech formant structure survives noise reduction.
            if self.formant_preserver.enabled {
                self.apply_formant_preservation(ch);
            }

            // Pass 1d: 3-bin median filter to suppress isolated musical noise
            // spikes. Applied before spectral/temporal smoothing so that the
            // smoother operates on clean gain curves.
            Self::median_smooth_gains(&mut self.gain[ch], self.spectrum_size);

            // Pass 1e: Harmonic/percussive differential denoising
            // Tonal bins get stronger denoising (noise is diffuse), transient bins get gentler
            if self.harmonic_percussive {
                // Compute magnitudes for separator
                for k in 0..self.spectrum_size {
                    self.tt_magnitudes[k] = self.get_power_at_bin(ch, k).sqrt();
                }
                self.tonal_transient_seps[ch].process(
                    &self.tt_magnitudes[..self.spectrum_size],
                    &mut self.tt_tonal_mask[..self.spectrum_size],
                    &mut self.tt_transient_mask[..self.spectrum_size],
                );
                for k in 0..self.spectrum_size {
                    // Transient-dominant bins: preserve attacks by blending gain toward 1.0.
                    // A weight of 1.0 means fully transient → keep the signal as-is (gain=1.0).
                    // A weight of 0.0 means fully tonal → apply the computed Wiener gain unchanged.
                    // The 0.5 factor gives a half-strength blend at full transient weight.
                    let transient_weight = self.tt_transient_mask[k];
                    let t = transient_weight * 0.5;
                    self.gain[ch][k] = self.gain[ch][k] * (1.0 - t) + t;
                }
            }

            // Pass 2: Smooth gains across frequency bins
            if self.spectral_smoothing_enabled {
                self.smooth_gains_across_frequency(ch);
            }

            // Pass 2b: Psychoacoustic masking — skip denoising for masked bins.
            // Guard: only apply masking when speech presence probability is above a
            // minimum threshold (0.1). On noise-only frames the signal power equals
            // the noise power, so the noise can "mask itself" (especially at low
            // frequencies where Bark spreading is wide), incorrectly bypassing
            // denoising on bins that should be attenuated.
            if self.psychoacoustic_masking {
                self.compute_masking_thresholds(ch);
                for k in 0..self.spectrum_size {
                    if self.speech_presence[ch][k] >= 0.1 && self.is_noise_masked(ch, k) {
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

        // Spatial denoising (after all per-channel passes complete)
        // Applied to both channels simultaneously using inter-channel coherence
        if self.spatial_denoise && self.channels >= 2 {
            let strength = self.spatial_strength;
            for k in 0..self.spectrum_size {
                let p0 = self.get_power_at_bin(0, k);
                let p1 = self.get_power_at_bin(1, k);
                let coherence = if p0 + p1 > EPSILON {
                    2.0 * (p0 * p1).sqrt() / (p0 + p1)
                } else {
                    0.0
                };
                let incoherence = 1.0 - coherence;
                let extra_reduction = incoherence * strength * 0.3;
                self.smoothed_gain[0][k] =
                    (self.smoothed_gain[0][k] - extra_reduction).max(floor_linear);
                self.smoothed_gain[1][k] =
                    (self.smoothed_gain[1][k] - extra_reduction).max(floor_linear);
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

    /// In-place 3-bin median smoothing of gain values.
    ///
    /// Musical noise artifacts appear as isolated loud bins surrounded by
    /// quiet bins. A 3-bin median filter removes these narrow spikes without
    /// affecting broadband content. First and last elements are unchanged.
    ///
    /// Uses a sliding window that reads ahead by one bin, so no temporary
    /// buffer is needed — only a single `prev_in` variable tracks the
    /// pre-overwrite value of the previous bin.
    #[inline]
    pub(super) fn median_smooth_gains(gains: &mut [f32], len: usize) {
        if len < 3 {
            return;
        }

        let mut prev_in = gains[0];

        for i in 1..len - 1 {
            let a = prev_in;
            let b = gains[i];
            let c = gains[i + 1];
            prev_in = b; // save before overwrite

            // Median of three
            let median = if (a <= b && b <= c) || (c <= b && b <= a) {
                b
            } else if (b <= a && a <= c) || (c <= a && a <= b) {
                a
            } else {
                c
            };

            gains[i] = median;
        }
        // Last element stays unchanged
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

    /// Smooth gains across frequency bins using a 5-tap Gaussian kernel with
    /// adaptive width based on local SNR.
    ///
    /// The base kernel is controlled by the `smoothing` parameter (sigma = smoothing * 2 bins).
    /// When adaptive smoothing is active, a second wider pass is blended in for low-SNR bins:
    /// - High SNR (clean signal): narrow smoothing preserves spectral detail
    /// - Low SNR (noisy): wider smoothing reduces musical noise artifacts
    ///
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

        // Adaptive pass: blend toward wider smoothing at low-SNR bins.
        // We run a second wider kernel (7-tap, double sigma) over the narrow-smoothed
        // result, then per-bin blend based on local SNR.
        // SNR threshold: bins with SNR < 6 dB (power ratio < 4) get more wide smoothing.
        self.apply_adaptive_spectral_smoothing(channel);
    }

    /// Apply adaptive spectral smoothing: blend toward wider smoothing for low-SNR bins.
    ///
    /// Uses a 7-tap uniform average as the "wide" kernel over the already narrow-smoothed
    /// gains. Per-bin blend factor is derived from local SNR:
    /// - SNR >= snr_high: keep narrow result (blend = 0)
    /// - SNR <= snr_low: use wide result (blend = 1)
    /// - Between: linear interpolation
    fn apply_adaptive_spectral_smoothing(&mut self, channel: usize) {
        let n = self.spectrum_size;

        // SNR thresholds for adaptive blend (in linear power ratio)
        // 3 dB = power ratio ~2, 10 dB = power ratio ~10
        let snr_low = 2.0_f32; // Below this: maximum wide smoothing
        let snr_high = 10.0_f32; // Above this: no extra smoothing
        let snr_range_inv = 1.0 / (snr_high - snr_low);

        // Copy narrow-smoothed gains to scratch for wide smoothing input
        self.freq_smooth_temp[..n].copy_from_slice(&self.gain[channel][..n]);

        // Access freq_domain and noise_psd directly to avoid borrow conflicts
        // with self.gain[channel].
        let radius: usize = 3;
        let use_captured = self.use_captured_profile && self.has_noise_profile;

        for k in 0..n {
            // Inline get_power_at_bin and get_effective_noise_power to avoid
            // borrowing &self while gain[channel] is mutably borrowed.
            let signal_power = self.freq_domain[channel][k].norm_sqr();
            let noise_power = if use_captured {
                self.noise_profile_storage[channel][k].max(EPSILON)
            } else {
                self.noise_psd[channel][k].max(EPSILON)
            };
            let local_snr = signal_power / noise_power;

            // Compute blend factor: 1.0 at low SNR, 0.0 at high SNR
            let blend = ((snr_high - local_snr) * snr_range_inv).clamp(0.0, 1.0);

            if blend < 0.001 {
                // High SNR: keep narrow-smoothed result as-is
                continue;
            }

            // Compute wide-smoothed value (box filter, radius 3) from scratch buffer
            let lo = k.saturating_sub(radius);
            let hi = (k + radius).min(n - 1);
            let count = (hi - lo + 1) as f32;
            let mut sum = 0.0_f32;
            for j in lo..=hi {
                sum += self.freq_smooth_temp[j];
            }
            let wide_val = sum / count;
            let narrow_val = self.freq_smooth_temp[k];

            // Blend narrow toward wide based on local SNR
            self.gain[channel][k] = narrow_val + blend * (wide_val - narrow_val);
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

    /// Apply formant preservation to Wiener gains for one channel.
    ///
    /// Must be called after the instantaneous Wiener gain is computed (Pass 1)
    /// and before spectral/temporal smoothing (Pass 2/3) so that the floored
    /// gains propagate through smoothing naturally.
    pub(super) fn apply_formant_preservation(&mut self, channel: usize) {
        if !self.formant_preserver.enabled {
            return;
        }

        // Compute log-magnitude for the current frame
        let n = self.spectrum_size;
        for k in 0..n {
            let power = self.freq_domain[channel][k].norm_sqr();
            // log10(power + ε) → log-magnitude (×10 gives power-dB, but ratio is
            // what matters here, so any consistent scaling works)
            self.formant_preserver.log_mag_scratch[k] = fast_log10(power.max(EPSILON));
        }

        // Estimate the spectral envelope via moving average of log-magnitude
        self.formant_preserver.estimate_envelope();

        // Floor gains at formant peaks
        let strength = self.formant_preserver.strength;
        let envelope = &self.formant_preserver.envelope;
        let mean_env: f32 = envelope.iter().sum::<f32>() / n as f32;
        let floor_gain = strength * 0.3;

        for (env_val, gain_val) in envelope.iter().zip(self.gain[channel].iter_mut()).take(n) {
            if *env_val > mean_env + 0.13 {
                // 0.13 in log10(power) units = 1.3 dB above mean (10 * 0.13 = 1.3 dB)
                *gain_val = gain_val.max(floor_gain);
            }
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

// ============================================================================
// FormantPreserver
// ============================================================================

/// Preserves speech formant peaks by flooring Wiener gains at spectral envelope peaks.
pub(super) struct FormantPreserver {
    /// Pre-allocated scratch buffer for log-magnitude spectrum
    pub log_mag_scratch: Vec<f32>,
    /// Smoothed spectral envelope in log-magnitude domain
    pub envelope: Vec<f32>,
    /// Scratch copy of envelope for backward pass (avoids reading stale data)
    envelope_scratch: Vec<f32>,
    /// Smoothing window half-width in bins (= lifter_len)
    lifter_len: usize,
    /// Whether formant preservation is active
    pub enabled: bool,
    /// Preservation strength [0.0, 1.0]
    pub strength: f32,
}

impl FormantPreserver {
    /// Allocate buffers for a given spectrum size.
    pub fn new(spectrum_size: usize) -> Self {
        Self {
            log_mag_scratch: vec![0.0_f32; spectrum_size],
            envelope: vec![0.0_f32; spectrum_size],
            envelope_scratch: vec![0.0_f32; spectrum_size],
            lifter_len: LIFTER_LEN,
            enabled: false,
            strength: 0.5,
        }
    }

    /// Estimate the spectral envelope from `self.log_mag_scratch` using a
    /// causal moving average with window width `lifter_len * 2`.
    ///
    /// The moving average over log-magnitude is equivalent to a smoothed
    /// spectral envelope: wide windows suppress harmonics and retain only
    /// slowly-varying peaks (formants), matching the effect of a low-pass
    /// lifter in the cepstral domain.
    ///
    /// We use a two-pass (forward + backward) box filter to produce a
    /// symmetric (zero-phase) result from the causal accumulators.
    pub fn estimate_envelope(&mut self) {
        let n = self.log_mag_scratch.len();
        let win = (self.lifter_len * 2).min(n);

        // Forward pass: running sum → forward-smoothed values stored in envelope
        let mut running_sum = 0.0_f32;
        let mut count = 0usize;
        for k in 0..n {
            running_sum += self.log_mag_scratch[k];
            count += 1;
            if k >= win {
                running_sum -= self.log_mag_scratch[k - win];
                count -= 1;
            }
            self.envelope[k] = running_sum / count as f32;
        }

        // Backward pass: average the forward-smoothed result with a mirrored pass
        // to cancel the lag introduced by the causal forward accumulator.
        // Copy forward-pass result to scratch to avoid reading stale overwritten data.
        self.envelope_scratch[..n].copy_from_slice(&self.envelope[..n]);
        running_sum = 0.0;
        count = 0;
        for k in (0..n).rev() {
            running_sum += self.envelope_scratch[k];
            count += 1;
            if n - 1 - k >= win {
                running_sum -= self.envelope_scratch[k + win];
                count -= 1;
            }
            // Average forward and backward estimates
            self.envelope[k] = (self.envelope_scratch[k] + running_sum / count as f32) * 0.5;
        }
    }
}
